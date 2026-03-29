use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub type LevelId = String;

/// Données d’un niveau : effectif et, optionnellement, liste nominative des élèves.
#[derive(Debug, Clone)]
pub struct LevelData {
    pub count: u32,
    /// Vide = effectif anonyme (seul `count` compte pour l’optimisation).
    pub students: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum LevelInput {
    Count(u32),
    Detailed {
        #[serde(default)]
        count: Option<u32>,
        #[serde(default)]
        students: Vec<String>,
    },
}

#[derive(Debug, Deserialize)]
struct RawConfig {
    #[serde(flatten)]
    plan: PlanConfig,
    levels: HashMap<LevelId, LevelInput>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlanConfig {
    /// Nombre de classes (postes / enseignants).
    pub num_classes: usize,
    /// Effectif maximum par classe (sauf surcharge si `max_class_size` absent par niveau).
    #[serde(default = "default_max_class")]
    pub max_students_per_class: u32,
    /// Effectif minimum souhaité par classe (0 = pas de contrainte dure).
    #[serde(default)]
    pub min_students_per_class: u32,
    /// Effectif minimum **par niveau** dans une classe à double niveau (ex. 3 pour refuser un duo 1 CM2 + 29 CM1).
    #[serde(default = "default_min_per_level_dual")]
    pub min_students_per_level_in_dual_class: u32,
    /// Nombre de plans à produire (5–10 typiquement).
    #[serde(default = "default_num_plans")]
    pub num_plans: usize,
    /// Budget de nœuds pour le DFS (sécurité).
    #[serde(default = "default_dfs_budget")]
    pub dfs_node_budget: usize,
    /// Essais aléatoires complémentaires.
    #[serde(default = "default_random_trials")]
    pub random_trials: usize,
    /// Ordre des niveaux (adjacence = index consécutifs).
    #[serde(default)]
    pub level_order: Vec<LevelId>,
    /// Paires interdites pour un double niveau (noms de niveaux).
    #[serde(default)]
    pub forbidden_pairs: Vec<(LevelId, LevelId)>,
    /// Groupes de « même cycle » : bonus si la paire est dans un même groupe.
    #[serde(default)]
    pub same_cycle_groups: Vec<Vec<LevelId>>,
    #[serde(default)]
    pub scoring: ScoringWeights,
}

fn default_max_class() -> u32 {
    25
}

fn default_min_per_level_dual() -> u32 {
    2
}

fn default_num_plans() -> usize {
    8
}

fn default_dfs_budget() -> usize {
    250_000
}

fn default_random_trials() -> usize {
    30_000
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScoringWeights {
    #[serde(default = "default_w_distance")]
    pub distance_weight: f64,
    #[serde(default = "default_w_cross")]
    pub cross_cycle_penalty: f64,
    #[serde(default = "default_w_balance")]
    pub balance_weight: f64,
    #[serde(default)]
    pub overload_penalty_weight: f64,
}

impl Default for ScoringWeights {
    fn default() -> Self {
        Self {
            distance_weight: default_w_distance(),
            cross_cycle_penalty: default_w_cross(),
            balance_weight: default_w_balance(),
            overload_penalty_weight: 100.0,
        }
    }
}

fn default_w_distance() -> f64 {
    3.0
}

fn default_w_cross() -> f64 {
    15.0
}

fn default_w_balance() -> f64 {
    1.0
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub plan: PlanConfig,
    pub levels: HashMap<LevelId, LevelData>,
}

impl AppConfig {
    pub fn load_path(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("lecture du fichier {:?}", path))?;
        let raw_cfg: RawConfig = toml::from_str(&raw).context("parse TOML")?;
        Self::from_raw(raw_cfg)
    }

    fn from_raw(raw: RawConfig) -> Result<Self> {
        let mut cfg = AppConfig {
            plan: raw.plan,
            levels: HashMap::new(),
        };
        for (name, input) in raw.levels {
            let data = level_input_to_data(&name, input)?;
            cfg.levels.insert(name, data);
        }
        cfg.normalize()?;
        Ok(cfg)
    }

    /// Au moins un niveau comporte une liste nominative d’élèves.
    pub fn has_named_students(&self) -> bool {
        self.levels.values().any(|d| !d.students.is_empty())
    }

    fn normalize(&mut self) -> Result<()> {
        if self.plan.num_classes == 0 {
            return Err(anyhow!("num_classes doit être ≥ 1"));
        }
        if self.levels.is_empty() {
            return Err(anyhow!("au moins un niveau doit être défini dans [levels]"));
        }
        if self.plan.level_order.is_empty() {
            let mut names: Vec<_> = self.levels.keys().cloned().collect();
            names.sort();
            self.plan.level_order = names;
        }
        let order_set: HashSet<_> = self.plan.level_order.iter().collect();
        for name in self.levels.keys() {
            if !order_set.contains(name) {
                return Err(anyhow!(
                    "le niveau {:?} n’est pas dans level_order",
                    name
                ));
            }
        }
        let total: u64 = self.levels.values().map(|d| d.count as u64).sum();
        let cap = self.plan.num_classes as u64 * self.plan.max_students_per_class as u64;
        if total > cap {
            return Err(anyhow!(
                "effectif total {} dépasse la capacité {} classes × {} élèves",
                total,
                self.plan.num_classes,
                self.plan.max_students_per_class
            ));
        }

        let mdl = self.plan.min_students_per_level_in_dual_class;
        if mdl < 1 {
            return Err(anyhow!(
                "min_students_per_level_in_dual_class doit être ≥ 1"
            ));
        }
        if mdl * 2 > self.plan.max_students_per_class {
            return Err(anyhow!(
                "min_students_per_level_in_dual_class ({}) est trop grand : il faut 2 × ce minimum ≤ max_students_per_class ({}) pour toute classe à double niveau",
                mdl,
                self.plan.max_students_per_class
            ));
        }

        let mut seen_names = HashSet::new();
        for (level, data) in &self.levels {
            for s in &data.students {
                if s.trim().is_empty() {
                    return Err(anyhow!(
                        "nom d’élève vide dans le niveau {:?}",
                        level
                    ));
                }
                if !seen_names.insert(s.as_str()) {
                    return Err(anyhow!("élève en double dans les listes : {:?}", s));
                }
            }
        }

        Ok(())
    }

    pub fn level_index(&self, id: &str) -> Option<usize> {
        self.plan.level_order.iter().position(|x| x == id)
    }

    pub fn pair_cost(&self, a: &str, b: &str) -> f64 {
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        let w = &self.plan.scoring;
        let mut cost = 0.0;
        if let (Some(ia), Some(ib)) = (self.level_index(a), self.level_index(b)) {
            let d = (ia as isize - ib as isize).unsigned_abs();
            if d > 1 {
                cost += w.distance_weight * (d - 1) as f64;
            }
        } else {
            cost += w.distance_weight * 5.0;
        }
        let same_cycle = self.plan.same_cycle_groups.iter().any(|g| {
            g.iter().any(|x| x == a) && g.iter().any(|x| x == b)
        });
        if !same_cycle {
            cost += w.cross_cycle_penalty;
        }
        cost
    }

    pub fn is_forbidden_pair(&self, a: &str, b: &str) -> bool {
        self.plan.forbidden_pairs.iter().any(|(x, y)| {
            (x == a && y == b) || (x == b && y == a)
        })
    }
}

fn level_input_to_data(level_name: &str, input: LevelInput) -> Result<LevelData> {
    match input {
        LevelInput::Count(n) => Ok(LevelData {
            count: n,
            students: vec![],
        }),
        LevelInput::Detailed { count, students } => {
            if !students.is_empty() {
                let n = students.len() as u32;
                if let Some(c) = count {
                    if c != n {
                        return Err(anyhow!(
                            "niveau {:?} : count ({}) ≠ nombre d’élèves listés ({})",
                            level_name,
                            c,
                            n
                        ));
                    }
                }
                Ok(LevelData {
                    count: n,
                    students,
                })
            } else {
                let c = count.ok_or_else(|| {
                    anyhow!(
                        "niveau {:?} : indiquez soit un entier (effectif), soit {{ count = … }} ou {{ students = […] }}",
                        level_name
                    )
                })?;
                Ok(LevelData {
                    count: c,
                    students: vec![],
                })
            }
        }
    }
}
