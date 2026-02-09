# Système de Skills LocaLM

Le système de skills de LocaLM est inspiré de Claude Code et suit le standard [Agent Skills](https://agentskills.io). Il permet d'ajouter facilement des capacités spécialisées à l'IA.

## 📁 Structure des Skills

```
.localm/skills/                    # Skills du projet (commitables)
├── playwright/                    # Skill pour tests navigateur
│   ├── SKILL.md                   # Fichier principal (obligatoire)
│   ├── examples.md                # Exemples d'utilisation (optionnel)
│   └── templates/                 # Templates (optionnel)
│       └── test-template.ts
├── git-master/
│   └── SKILL.md
└── rust-expert/
    └── SKILL.md

~/.localm/skills/                  # Skills globaux (utilisateur)
├── explain-code/
│   └── SKILL.md
└── debug-helper/
    └── SKILL.md
```

## 📝 Format SKILL.md

```yaml
---
name: playwright
description: Expert en tests navigateur avec Playwright. Utilise quand l'utilisateur demande de tester une interface web.
disable_auto_invoke: false
allowed_tools:
  - file_read
  - file_write
  - bash
---

# Playwright Skill

Tu es un expert en tests E2E avec Playwright.

## Règles
- Utilise TypeScript pour tous les tests
- Ajoute des commentaires explicatifs
- Utilise les sélecteurs les plus stables (data-testid)
- Implémente le pattern Page Object Model

## Exemple de structure
```typescript
import { test, expect } from '@playwright/test';

test('description', async ({ page }) => {
  await page.goto('/');
  await expect(page).toHaveTitle(/Expected Title/);
});
```
```

## 🛠️ Outils de Gestion des Skills

### 1. `skill_create` - Créer un skill

Permet à l'IA de créer de nouveaux skills.

**Paramètres :**
- `name` : Nom du skill (alphanumérique + tirets)
- `description` : Description de ce que fait le skill
- `content` : Instructions en markdown
- `is_global` : `true` pour skill global, `false` pour projet
- `disable_auto_invoke` : Désactiver l'invocation auto
- `allowed_tools` : Liste des outils autorisés (optionnel)

**Exemple :**
```json
{
  "name": "react-expert",
  "description": "Expert React/TypeScript. Utilise pour les composants React.",
  "content": "Tu es un expert React...",
  "is_global": false,
  "allowed_tools": ["file_read", "file_write", "grep"]
}
```

### 2. `skill_invoke` - Invoquer un skill

Active un skill spécifique pour la conversation en cours.

**Paramètres :**
- `name` : Nom du skill à invoquer

**Exemples :**
- `/playwright` → Invoque le skill playwright
- `/react-expert` → Invoque le skill react-expert

### 3. `skill_list` - Lister les skills

Affiche tous les skills disponibles avec leurs descriptions.

## 🎯 Utilisation

### Invocation Directe (Slash Commands)

Dans le chat, tape `/` suivi du nom du skill :

```
/playwright
```

L'IA chargera alors les instructions du skill et les appliquera.

### Invocation Automatique

L'IA peut charger automatiquement un skill si la description correspond à la requête :

- User : "Crée un test pour cette page"
- IA : Détecte le skill "playwright" et l'active automatiquement

### Création via l'IA

Demande à l'IA de créer un skill :

```
Crée un skill "docker-expert" qui me donne les meilleures pratiques Docker
```

L'IA utilisera `skill_create` pour générer le SKILL.md.

## 📂 Emplacements des Skills

| Type | Chemin | Description |
|------|--------|-------------|
| **Projet** | `./.localm/skills/<name>/` | Spécifique au projet, commitable |
| **Global** | `~/.localm/skills/<name>/` | Disponible dans tous les projets |

### Chemins par OS

- **Windows** : `%APPDATA%\LocaLM\skills\`
- **macOS** : `~/Library/Application Support/LocaLM/skills/`
- **Linux** : `~/.local/share/LocaLM/skills/`

## 🔧 Exemples de Skills

### 1. Skill Playwright

```yaml
---
name: playwright
description: Expert en tests E2E avec Playwright. Utilise quand l'utilisateur demande de créer des tests navigateur.
---

Tu es un expert en tests automatisés avec Playwright.

## Principes
- Utilise TypeScript
- Préfère les sélecteurs data-testid
- Ajoute des assertions explicites
- Structure avec Page Object Model

## Pattern recommandé
```typescript
// pages/LoginPage.ts
export class LoginPage {
  constructor(private page: Page) {}
  
  async login(email: string, password: string) {
    await this.page.fill('[data-testid="email"]', email);
    await this.page.fill('[data-testid="password"]', password);
    await this.page.click('[data-testid="submit"]');
  }
}
```
```

### 2. Skill Git Master

```yaml
---
name: git-master
description: Expert Git. Utilise pour les opérations Git complexes ou la résolution de conflits.
---

Tu es un expert Git avec 10 ans d'expérience.

## Workflow recommandé
1. Vérifier l'état : `git status`
2. Vérifier les branches : `git branch -a`
3. Créer des commits atomiques
4. Utiliser rebase interactif pour l'historique propre

## Commandes favorites
```bash
# Historique graphique
git log --graph --oneline --all

# Rebase interactif
git rebase -i HEAD~3
```
```

### 3. Skill Rust Expert

```yaml
---
name: rust-expert
description: Expert Rust. Utilise pour optimiser le code Rust ou expliquer les concepts avancés.
---

Tu es un expert Rust avec une maîtrise des lifetimes, ownership et patterns avancés.

## Règles
- Utilise `?` pour la propagation d'erreurs
- Préfère les enums aux booléens
- Documente les fonctions publiques avec `///`
- Utilise `thiserror` pour les erreurs

## Patterns
```rust
// Error handling
#[derive(Debug, thiserror::Error)]
pub enum MyError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// Result type
pub type Result<T> = std::result::Result<T, MyError>;
```
```

## 🚀 Meilleures Pratiques

### 1. Nommage
- Utilise des noms courts et descriptifs
- Préfère les tirets : `react-expert`, pas `react_expert`
- Évite les noms génériques comme `helper` ou `utils`

### 2. Description
- Sois précis sur QUAND utiliser le skill
- Inclus des mots-clés déclencheurs
- Garde sous 1024 caractères

**Bon exemple :**
```yaml
description: Expert en API REST Django. Utilise quand l'utilisateur demande de créer/modifier des endpoints, serializers ou vues Django REST.
```

**Mauvais exemple :**
```yaml
description: Aide avec Django  # Trop vague
```

### 3. Contenu
- Commence par définir le rôle de l'IA
- Liste les règles spécifiques
- Inclus des exemples de code
- Structure avec des titres clairs

### 4. Organisation
- Garde SKILL.md sous 500 lignes
- Déplace les exemples longs dans `examples.md`
- Utilise `templates/` pour les snippets réutilisables

## 🔄 Cycle de Vie

1. **Création** : Via `skill_create` ou manuellement
2. **Découverte** : Chargement au démarrage de l'agent
3. **Invocation** : Manuelle (`/skill-name`) ou automatique
4. **Application** : Instructions injectées dans le contexte
5. **Mise à jour** : Modifiez le SKILL.md, rechargement auto

## 🎓 Apprentissage Progressif

Commencez avec des skills simples et enrichissez-les :

**V1 - Basique :**
```yaml
---
name: python-expert
description: Expert Python
---

Tu es un expert Python.
```

**V2 - Amélioré :**
```yaml
---
name: python-expert
description: Expert Python. Utilise pour le code Python, les data structures, ou les questions sur asyncio.
---

Tu es un expert Python avec 10 ans d'expérience.

## Standards
- Type hints obligatoires
- Docstrings Google style
- Tests avec pytest

## Patterns
```python
# Type hints
def process(data: list[dict]) -> list[Result]:
    ...
```
```

**V3 - Avancé :**
- Ajoute des exemples complexes
- Inclus des règles métier
- Références à d'autres fichiers

## 📚 Skills Recommandés

Voici une liste de skills utiles à créer :

1. **playwright** - Tests E2E navigateur
2. **git-master** - Expert Git avancé
3. **docker-expert** - Conteneurisation
4. **ci-cd** - Pipelines CI/CD
5. **security** - Bonnes pratiques sécurité
6. **performance** - Optimisation performance
7. **testing** - Stratégies de test
8. **database** - Design et requêtes SQL
9. **api-design** - Design d'APIs REST/GraphQL
10. **refactoring** - Patterns de refactoring

## 🔒 Permissions

Les skills héritent du système de permissions existant :
- `skill_create` : Niveau WriteFile (crée des fichiers)
- `skill_invoke` : Niveau ReadOnly (lecture seule)
- `skill_list` : Niveau ReadOnly (lecture seule)

## 💡 Astuces

1. **Skills imbriqués** : Un skill peut invoquer un autre skill
2. **Variables** : Utilisez `$ARGUMENTS` pour passer des paramètres (futur)
3. **Templates** : Stockez les templates dans le dossier du skill
4. **Versioning** : Committez vos skills dans Git pour les partager

---

## 🎉 Démarrage Rapide

1. **Crée ton premier skill :**
```
Crée un skill "mon-helper" qui me rappelle d'ajouter des tests pour chaque nouvelle fonction
```

2. **Utilise-le :**
```
/mon-helper
```

3. **Liste tes skills :**
```
Liste tous les skills disponibles
```

Et voilà ! Ton IA a maintenant de nouvelles capacités 🚀
