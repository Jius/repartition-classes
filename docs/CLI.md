# Guide d’utilisation de la ligne de commande

`repartition-classes` est un binaire unique : il lit un fichier **TOML** de configuration, calcule plusieurs plans de répartition, puis écrit le résultat sur **la sortie standard** (`stdout`).

## Installation du binaire

Depuis la racine du projet :

```bash
cargo build --release
```

Le binaire se trouve dans :

```text
target/release/repartition-classes
```

Vous pouvez le copier dans un répertoire du `PATH`, ou l’invoquer avec le chemin complet.

## Aperçu

```bash
repartition-classes --input chemin/vers/config.toml
```

Sans option supplémentaire, la sortie est au **format texte** (tableaux ASCII).

## Aide intégrée

```bash
repartition-classes --help
```

Affiche le résumé des options et les valeurs possibles pour `--format`.

## Options

| Option | Court | Obligatoire | Défaut | Description |
|--------|-------|-------------|--------|-------------|
| `--input` | `-i` | oui | — | Chemin vers le fichier **TOML** (effectifs, `num_classes`, contraintes, listes d’élèves, etc.). |
| `--format` | — | non | `text` | Format de sortie : `text`, `markdown` ou `json`. |

### `--input` (`-i`)

Chemin relatif ou absolu. Le fichier doit être un TOML valide conforme au schéma décrit dans le [README](../README.md) (section « Format d’entrée »).

Exemples :

```bash
repartition-classes -i examples/ecole_elementaire.toml
repartition-classes --input /Users/moi/ecole/rentree_2026.toml
```

### `--format`

Contrôle la représentation des plans sur la sortie standard.

| Valeur | Usage typique |
|--------|----------------|
| `text` | Lecture directe dans le terminal ; tableaux ASCII et listes d’élèves indentées sous chaque classe. |
| `markdown` | Copier-coller dans un document ou un outil qui rend le Markdown (titres, tableaux, listes à puces sous les classes). |
| `json` | Chaînage avec `jq`, scripts, import dans un tableur ou une appli ; structure stable (`plans[]` avec `classes`, `assignment`, `metrics`, `students_per_class` si listes fournies). |

Exemples :

```bash
repartition-classes -i examples/avec_listes_eleves.toml --format text
repartition-classes -i examples/avec_listes_eleves.toml --format markdown
repartition-classes -i examples/avec_listes_eleves.toml --format json > plans.json
```

## Redirection et pipe

La sortie va toujours sur **stdout** ; les messages d’erreur vont sur **stderr**.

Enregistrer le résultat dans un fichier :

```bash
repartition-classes -i config.toml --format json > sortie/plans.json
repartition-classes -i config.toml --format markdown > reunion/plans.md
```

Filtrer le JSON (si `jq` est installé) :

```bash
repartition-classes -i config.toml --format json | jq '.[0].metrics.global_score'
```

## Codes de sortie

- **0** : exécution terminée sans erreur (y compris lorsque **aucun plan** n’a été trouvé : le programme affiche alors un message explicite et quitte quand même avec 0).
- **≠ 0** : erreur (fichier introuvable, TOML invalide, contraintes de configuration rejetées, etc.). Le détail est affiché sur stderr.

Au démarrage, une ligne sur **stderr** rappelle la valeur chargée de `min_students_per_level_in_dual_class` (pour vérifier que le TOML attendu est bien pris en compte). La sortie des plans reste sur **stdout** pour pouvoir rediriger proprement (`> fichier.json`).

## Développement (sans installation release)

```bash
cargo run -- --input examples/ecole_elementaire.toml
cargo run -- -i examples/avec_listes_eleves.toml --format json
```

Le `--` sépare les arguments de `cargo` de ceux du programme.

## Rappel

Les **paramètres métier** (nombre de classes, effectifs, listes d’élèves, pénalités, **minimum d’élèves par niveau dans un double niveau**, etc.) ne sont **pas** des options de la CLI : ils se configurent uniquement dans le fichier TOML passé à `--input` (voir le [README](../README.md)).
