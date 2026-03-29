# repartition-classes

Outil en ligne de commande (Rust) pour aider une équipe à **proposer plusieurs plans** de répartition des élèves du primaire en classes **simples ou doubles niveaux**, avec un **score** reflétant la proximité pédagogique des regroupements et l’équilibre des effectifs.

## Hypothèses par défaut (modifiables dans le fichier TOML)

- **Niveaux** : noms libres (ex. `CP`, `CE1`, … ou `PS`, `MS`, `GS`). Si `level_order` est omis, l’ordre est l’ordre **alphabétique** des clés de `[levels]` (à configurer explicitement pour une adjacence fiable).
- **Élèves** : pour chaque niveau vous pouvez donner soit un **entier** (effectif anonyme), soit une **liste de noms** dans `[levels]` ; les sorties reprennent alors les listes **réparties par classe** (ordre des classes 1…K, puis ordre d’origine dans la liste du niveau). Les noms doivent être **uniques** sur toute l’école.
- **Classes** : `num_classes` = nombre de postes / enseignants ; **chaque classe est utilisée** (effectif total > 0 par classe).
- **Taille** : `max_students_per_class` (défaut 25) est une **contrainte dure** ; `min_students_per_class` est optionnel (0 = désactivé).
- **Double niveau** : exactement **deux** niveaux par classe concernée. Le paramètre **`min_students_per_level_in_dual_class`** impose au moins ce nombre d’élèves **par niveau** dans chaque classe à double niveau. **Défaut : 2** (évite les compositions « 1 + N » : avec la valeur 1, le flot pouvait ne laisser qu’**un** élève sur un niveau et tout le reste sur l’autre). Pour être encore plus strict (ex. au moins 5 de chaque côté), augmentez cette valeur. Il faut `2 × cette valeur ≤ max_students_per_class`. Au lancement, la valeur réellement chargée est rappelée sur **stderr**.
- **Paires interdites** : liste `forbidden_pairs` (tableau de paires de chaînes).
- **Cycles** : `same_cycle_groups` permet de regrouper des niveaux ; une paire hors de tout groupe commun reçoit une pénalité `cross_cycle_penalty` dans le score.

## Modèle et score

- Une **composition** est la liste des `num_classes` classes ; chaque classe contient 1 ou 2 niveaux.
- La **faisabilité** (répartition entière des effectifs respectant le plafond par classe et la règle du double niveau) est vérifiée par un **flot maximum** (Dinic).
- Le **score** (plus bas = mieux) combine :
  - **Proximité** dans `level_order` : pénalité si les deux niveaux ne sont pas adjacents (`distance_weight` × écart d’index − 1).
  - **Cycle** : `cross_cycle_penalty` si les deux niveaux ne partagent aucun groupe dans `same_cycle_groups`.
  - **Équilibre** : `balance_weight` × variance des effectifs par classe.
  - **Surcharge** : pénalité si une classe dépasse `max_students_per_class` (normalement absent si le flot réussit).

## Algorithme

1. **DFS** avec élagage (budget `dfs_node_budget`) pour énumérer des compositions cohérentes (couverture des niveaux, contraintes sur le nombre minimal de classes par niveau).
2. **Tirages aléatoires** (`random_trials`) pour diversifier les plans lorsque l’espace est vaste.
3. Conservation des **meilleurs plans distincts** (à structure près), jusqu’à `num_plans` (plafonné à 20 dans l’implémentation actuelle).

Il n’y a **pas de garantie d’optimalité globale** : l’objectif est d’offrir **5 à 10 alternatives** de bonne qualité pour discussion en équipe.

## Installation et usage

```bash
cd repartition-classes
cargo build --release
./target/release/repartition-classes --input examples/ecole_elementaire.toml
```

**Documentation détaillée de la CLI** (options, formats, redirection, codes de sortie) : voir [docs/CLI.md](docs/CLI.md).

En résumé :

- `-i` / `--input` : fichier TOML (obligatoire).
- `--format` : `text` (défaut), `markdown` ou `json`.

```bash
./target/release/repartition-classes --help
```

## Format d’entrée (TOML)

Les paramètres de plan et les effectifs sont **à la racine** (champs aplatis) :

| Champ | Rôle |
|--------|------|
| `num_classes` | Nombre de classes |
| `max_students_per_class` | Plafond par classe |
| `min_students_per_class` | Effectif minimum (0 = inactif) |
| `min_students_per_level_in_dual_class` | Minimum d’élèves **par niveau** dans toute classe à double niveau (défaut **2**) |
| `num_plans` | Nombre de plans à afficher (ex. 8) |
| `dfs_node_budget` | Limite de nœuds DFS |
| `random_trials` | Essais aléatoires complémentaires |
| `level_order` | Ordre des niveaux pour l’adjacence |
| `forbidden_pairs` | `[[ "A", "B" ], ...]` |
| `same_cycle_groups` | `[ [ "CP", "CE1" ], [ "CM1", "CM2" ] ]` |
| `[scoring]` | Sous-table des poids (optionnel, défauts raisonnables) |
| `[levels]` | Voir ci-dessous |

### Table `[levels]`

Trois formes par niveau (TOML) :

- Effectif seul : `CP = 24`
- Liste nominative (l’effectif est `len(students)`) :

  `CE1 = { students = ["Alice Martin", "Bob Durand", "Chloé"] }`

- Effectif explicite qui doit coïncider avec la liste :

  `CE2 = { count = 2, students = ["Diane", "Eve"] }`

Vous pouvez **mélanger** niveaux anonymes (`CP = 24`) et niveaux nominatifs sur le même fichier. Les niveaux sans liste n’apparaissent pas dans `students_per_class` (JSON) mais restent comptés dans les effectifs.

Voir `examples/ecole_elementaire.toml` (effectifs simples) et `examples/avec_listes_eleves.toml` (exemple avec noms).

## Exemple de sortie (extrait)

Avec le fichier d’exemple, la commande affiche plusieurs plans avec, pour chacun : score global, nombre de doubles niveaux, paires concernées, moyenne / écart-type des effectifs, puis le détail classe par classe (niveau(x), total, répartition par niveau).

Sortie JSON : tableau de plans avec `classes`, `assignment` (`per_class`, `class_totals`), `metrics`, et si des listes ont été fournies : `students_per_class` (une entrée par classe, puis par niveau la liste des noms affectés).

## Tests

```bash
cargo test
```

## Limites connues

- Pas de prise en charge fine des **besoins particuliers**, des sections, des langues, etc. (à étendre au besoin).
- Pas de contrainte du type « un niveau au plus dans *n* classes » au-delà de ce qu’imposent le plafond et le flot.
- L’exploration est bornée : des solutions valides peuvent être manquées si le budget DFS / les tirages aléatoires sont trop faibles pour votre taille d’école.
- Très grands `num_classes` et beaucoup de niveaux peuvent exploser la combinatoire ; augmenter `dfs_node_budget` et `random_trials` avec prudence.
- Si **un seul niveau** est réparti sur **plusieurs classes mononiveau**, le flot maximum peut regrouper trop d’élèves dans la première classe au sens de l’algorithme, ce qui fait échouer la contrainte « chaque classe non vide » : dans ce cas peu de plans (ou aucun) peuvent être trouvés ; ajouter d’autres niveaux ou des doubles niveaux aide à remplir toutes les classes.

## Licence

MIT OR Apache-2.0 (voir `Cargo.toml`).
