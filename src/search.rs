//! Recherche de compositions (mono / double niveau) par DFS + tirages aléatoires,
//! puis faisabilité et score via flot max.

use crate::config::{AppConfig, LevelId};
use crate::flow::{feasible_assignment, Assignment};
use rand::seq::SliceRandom;
use serde::Serialize;
use rand::Rng;
use std::collections::{HashMap, HashSet};

/// Niveaux présents dans une classe (1 ou 2 entrées, triées pour une paire).
pub type ClassComposition = Vec<LevelId>;

/// Par classe : pour chaque niveau ayant une liste en entrée, les noms affectés à cette classe.
pub type ClassRoster = HashMap<LevelId, Vec<String>>;

#[derive(Debug, Clone, Serialize)]
pub struct PlanMetrics {
    pub num_double_level_classes: usize,
    pub double_level_pairs: Vec<(LevelId, LevelId)>,
    pub class_totals: Vec<u32>,
    pub mean_class_size: f64,
    pub stdev_class_size: f64,
    pub max_deviation_from_mean: f64,
    pub global_score: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Plan {
    pub classes: Vec<ClassComposition>,
    pub assignment: Assignment,
    pub metrics: PlanMetrics,
    /// Présent dès qu’au moins un niveau a une liste d’élèves dans le fichier d’entrée.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub students_per_class: Option<Vec<ClassRoster>>,
}

pub fn search_plans(cfg: &AppConfig) -> Vec<Plan> {
    let active: Vec<LevelId> = cfg
        .levels
        .iter()
        .filter(|(_, d)| d.count > 0)
        .map(|(k, _)| k.clone())
        .collect();
    if active.is_empty() {
        return Vec::new();
    }
    let counts: HashMap<LevelId, u32> = cfg
        .levels
        .iter()
        .filter(|(_, d)| d.count > 0)
        .map(|(k, d)| (k.clone(), d.count))
        .collect();

    let k = cfg.plan.num_classes;
    let m = cfg.plan.max_students_per_class;
    let min_c = cfg.plan.min_students_per_class;
    let min_dual = cfg.plan.min_students_per_level_in_dual_class;

    let mut candidates: Vec<(Vec<Vec<LevelId>>, Assignment, f64)> = Vec::new();
    let mut seen_patterns: HashSet<String> = HashSet::new();

    let mut dfs_nodes = 0usize;
    let mut pattern_buf = vec![Vec::new(); k];
    dfs(
        cfg,
        &active,
        &counts,
        k,
        m,
        min_c,
        min_dual,
        0,
        &mut pattern_buf,
        &mut dfs_nodes,
        cfg.plan.dfs_node_budget,
        &mut candidates,
        &mut seen_patterns,
    );

    let mut rng = rand::thread_rng();
    random_search(
        cfg,
        &active,
        &counts,
        k,
        m,
        min_c,
        min_dual,
        cfg.plan.random_trials,
        &mut rng,
        &mut candidates,
        &mut seen_patterns,
    );

    candidates.sort_by(|a, b| {
        a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal)
    });

    let want = cfg.plan.num_plans.clamp(1, 20);
    select_diverse_best(candidates, want, cfg)
}

/// Répartit les listes d’élèves par classe dans l’ordre des classes (0..k) et de parcours des niveaux dans chaque composition.
fn build_named_roster(
    classes: &[Vec<LevelId>],
    assignment: &Assignment,
    cfg: &AppConfig,
) -> Option<Vec<ClassRoster>> {
    if !cfg.has_named_students() {
        return None;
    }
    let k = classes.len();
    let mut roster: Vec<ClassRoster> = (0..k).map(|_| HashMap::new()).collect();
    let mut cursor: HashMap<LevelId, usize> = HashMap::new();
    for (lvl, data) in &cfg.levels {
        if !data.students.is_empty() {
            cursor.insert(lvl.clone(), 0);
        }
    }
    for ci in 0..k {
        for lvl in &classes[ci] {
            let n = assignment
                .per_class
                .get(ci)
                .and_then(|m| m.get(lvl))
                .copied()
                .unwrap_or(0);
            if n == 0 {
                continue;
            }
            let data = cfg.levels.get(lvl)?;
            if data.students.is_empty() {
                continue;
            }
            let start = *cursor.get(lvl)?;
            let end = start + n as usize;
            if end > data.students.len() {
                return None;
            }
            roster[ci].insert(
                lvl.clone(),
                data.students[start..end].to_vec(),
            );
            *cursor.get_mut(lvl).unwrap() = end;
        }
    }
    for (lvl, data) in &cfg.levels {
        if data.students.is_empty() {
            continue;
        }
        let c = cursor.get(lvl).copied().unwrap_or(0);
        if c != data.students.len() {
            return None;
        }
    }
    Some(roster)
}

fn pattern_key(classes: &[Vec<LevelId>]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for comp in classes {
        let mut c = comp.clone();
        c.sort();
        parts.push(c.join("+"));
    }
    parts.sort();
    parts.join("|")
}

fn min_classes_for_level(n: u32, max_per: u32) -> usize {
    if n == 0 {
        return 0;
    }
    let m = max_per.max(1) as u64;
    (n as u64).div_ceil(m) as usize
}

fn appearances(pattern: &[Vec<LevelId>], depth_inclusive: usize, level: &str) -> usize {
    let end = depth_inclusive.min(pattern.len().saturating_sub(1));
    pattern
        .iter()
        .take(end + 1)
        .filter(|row| row.iter().any(|x| x == level))
        .count()
}

fn uncovered_levels(pattern: &[Vec<LevelId>], depth_exclusive: usize, active: &[LevelId]) -> Vec<LevelId> {
    active
        .iter()
        .filter(|l| {
            !pattern
                .iter()
                .take(depth_exclusive)
                .any(|row| row.iter().any(|x| x == *l))
        })
        .cloned()
        .collect()
}

fn prune(
    pattern: &[Vec<LevelId>],
    depth_after_place: usize,
    active: &[LevelId],
    counts: &HashMap<LevelId, u32>,
    k: usize,
    m: u32,
) -> bool {
    let remaining_slots = k.saturating_sub(depth_after_place);
    for l in active {
        let n = *counts.get(l).unwrap_or(&0);
        let need = min_classes_for_level(n, m);
        let app = appearances(pattern, depth_after_place.saturating_sub(1), l);
        if app + remaining_slots < need {
            return false;
        }
    }
    true
}

fn enumerate_options(
    cfg: &AppConfig,
    active: &[LevelId],
    pattern: &[Vec<LevelId>],
    depth: usize,
    k: usize,
) -> Vec<Vec<LevelId>> {
    let mut out = Vec::new();
    let rem_after = k - depth - 1;

    if depth == k - 1 {
        let unc = uncovered_levels(pattern, depth, active);
        match unc.len() {
            0 => {
                for l in active {
                    out.push(vec![l.clone()]);
                }
                for i in 0..active.len() {
                    for j in i + 1..active.len() {
                        let a = &active[i];
                        let b = &active[j];
                        if cfg.is_forbidden_pair(a, b) {
                            continue;
                        }
                        let mut v = vec![a.clone(), b.clone()];
                        v.sort();
                        out.push(v);
                    }
                }
            }
            1 => {
                out.push(vec![unc[0].clone()]);
            }
            2 => {
                let a = &unc[0];
                let b = &unc[1];
                if !cfg.is_forbidden_pair(a, b) {
                    let mut v = vec![a.clone(), b.clone()];
                    v.sort();
                    out.push(v);
                }
            }
            _ => return out,
        }
        return out;
    }

    for l in active {
        out.push(vec![l.clone()]);
    }
    for i in 0..active.len() {
        for j in i + 1..active.len() {
            let a = &active[i];
            let b = &active[j];
            if cfg.is_forbidden_pair(a, b) {
                continue;
            }
            let mut v = vec![a.clone(), b.clone()];
            v.sort();
            out.push(v);
        }
    }
    let _ = rem_after;
    out
}

#[allow(clippy::too_many_arguments)]
fn dfs(
    cfg: &AppConfig,
    active: &[LevelId],
    counts: &HashMap<LevelId, u32>,
    k: usize,
    m: u32,
    min_c: u32,
    min_dual: u32,
    depth: usize,
    pattern: &mut [Vec<LevelId>],
    dfs_nodes: &mut usize,
    budget: usize,
    candidates: &mut Vec<(Vec<Vec<LevelId>>, Assignment, f64)>,
    seen: &mut HashSet<String>,
) {
    *dfs_nodes += 1;
    if *dfs_nodes > budget {
        return;
    }
    if depth == k {
        if let Some(assign) = feasible_assignment(pattern, counts, m, min_c, min_dual) {
            if assign.class_totals.iter().all(|&t| t > 0) {
                let key = pattern_key(pattern);
                if seen.insert(key.clone()) {
                    let score = score_plan(cfg, pattern, &assign);
                    candidates.push((pattern.to_vec(), assign, score));
                }
            }
        }
        return;
    }

    let mut opts = enumerate_options(cfg, active, pattern, depth, k);
    opts.sort_by(|a, b| heuristic_comp(cfg, a).partial_cmp(&heuristic_comp(cfg, b)).unwrap());

    for comp in opts {
        pattern[depth] = comp.clone();
        if !prune(pattern, depth + 1, active, counts, k, m) {
            continue;
        }
        dfs(
            cfg,
            active,
            counts,
            k,
            m,
            min_c,
            min_dual,
            depth + 1,
            pattern,
            dfs_nodes,
            budget,
            candidates,
            seen,
        );
    }
}

fn heuristic_comp(cfg: &AppConfig, comp: &[LevelId]) -> f64 {
    match comp.len() {
        1 => 0.0,
        2 => cfg.pair_cost(&comp[0], &comp[1]),
        _ => 1000.0,
    }
}

#[allow(clippy::too_many_arguments)]
fn random_search<R: Rng + ?Sized>(
    cfg: &AppConfig,
    active: &[LevelId],
    counts: &HashMap<LevelId, u32>,
    k: usize,
    m: u32,
    min_c: u32,
    min_dual: u32,
    trials: usize,
    rng: &mut R,
    candidates: &mut Vec<(Vec<Vec<LevelId>>, Assignment, f64)>,
    seen: &mut HashSet<String>,
) {
    let opts = all_mono_dual_compositions(cfg, active);
    if opts.is_empty() {
        return;
    }
    for _ in 0..trials {
        let pattern: Vec<Vec<LevelId>> = (0..k)
            .map(|_| opts.choose(rng).unwrap().clone())
            .collect();
        let mut ok_cover = true;
        for l in active {
            if !pattern.iter().any(|c| c.iter().any(|x| x == l)) {
                ok_cover = false;
                break;
            }
        }
        if !ok_cover {
            continue;
        }
        if let Some(assign) = feasible_assignment(&pattern, counts, m, min_c, min_dual) {
            if assign.class_totals.iter().all(|&t| t > 0) {
                let key = pattern_key(&pattern);
                if seen.insert(key.clone()) {
                    let score = score_plan(cfg, &pattern, &assign);
                    candidates.push((pattern, assign, score));
                }
            }
        }
    }
}

fn all_mono_dual_compositions(cfg: &AppConfig, active: &[LevelId]) -> Vec<Vec<LevelId>> {
    let mut v = Vec::new();
    for l in active {
        v.push(vec![l.clone()]);
    }
    for i in 0..active.len() {
        for j in i + 1..active.len() {
            let a = &active[i];
            let b = &active[j];
            if cfg.is_forbidden_pair(a, b) {
                continue;
            }
            let mut t = vec![a.clone(), b.clone()];
            t.sort();
            v.push(t);
        }
    }
    v
}

fn score_plan(cfg: &AppConfig, composition: &[Vec<LevelId>], assign: &Assignment) -> f64 {
    let mut s = 0.0;
    for comp in composition {
        if comp.len() == 2 {
            s += cfg.pair_cost(&comp[0], &comp[1]);
        }
    }
    let totals = &assign.class_totals;
    if totals.is_empty() {
        return s;
    }
    let mean = totals.iter().map(|&x| x as f64).sum::<f64>() / totals.len() as f64;
    let var: f64 = totals
        .iter()
        .map(|&x| {
            let d = x as f64 - mean;
            d * d
        })
        .sum::<f64>()
        / totals.len() as f64;
    s += cfg.plan.scoring.balance_weight * var;
    for &t in totals {
        if t > cfg.plan.max_students_per_class {
            s += cfg.plan.scoring.overload_penalty_weight
                * (t - cfg.plan.max_students_per_class) as f64;
        }
    }
    s
}

fn build_metrics(classes: &[Vec<LevelId>], assign: &Assignment, score: f64) -> PlanMetrics {
    let mut pairs = Vec::new();
    let mut nd = 0usize;
    for comp in classes {
        if comp.len() == 2 {
            nd += 1;
            pairs.push((comp[0].clone(), comp[1].clone()));
        }
    }
    let totals = assign.class_totals.clone();
    let mean = if totals.is_empty() {
        0.0
    } else {
        totals.iter().map(|&x| x as f64).sum::<f64>() / totals.len() as f64
    };
    let var: f64 = if totals.is_empty() {
        0.0
    } else {
        totals
            .iter()
            .map(|&x| {
                let d = x as f64 - mean;
                d * d
            })
            .sum::<f64>()
            / totals.len() as f64
    };
    let stdev = var.sqrt();
    let max_dev = totals
        .iter()
        .map(|&x| (x as f64 - mean).abs())
        .fold(0.0_f64, f64::max);

    PlanMetrics {
        num_double_level_classes: nd,
        double_level_pairs: pairs,
        class_totals: totals,
        mean_class_size: mean,
        stdev_class_size: stdev,
        max_deviation_from_mean: max_dev,
        global_score: score,
    }
}

fn select_diverse_best(
    mut candidates: Vec<(Vec<Vec<LevelId>>, Assignment, f64)>,
    want: usize,
    cfg: &AppConfig,
) -> Vec<Plan> {
    candidates.sort_by(|a, b| {
        a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut out: Vec<Plan> = Vec::new();
    let mut keys = HashSet::new();
    for (classes, assign, score) in candidates {
        let k = pattern_key(&classes);
        if !keys.insert(k) {
            continue;
        }
        let metrics = build_metrics(&classes, &assign, score);
        let students_per_class = build_named_roster(&classes, &assign, cfg);
        out.push(Plan {
            classes: classes.clone(),
            assignment: assign,
            metrics,
            students_per_class,
        });
        if out.len() >= want {
            break;
        }
    }
    out
}

