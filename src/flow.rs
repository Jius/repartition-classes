//! Vérification de faisabilité et répartition entière via flot max (Dinic).

use crate::config::LevelId;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Default, Serialize)]
pub struct Assignment {
    /// class_idx -> (level -> count)
    pub per_class: Vec<HashMap<LevelId, u32>>,
    pub class_totals: Vec<u32>,
}

struct Dinic {
    n: usize,
    g: Vec<Vec<Edge>>,
}

struct Edge {
    to: usize,
    rev: usize,
    cap: i64,
}

impl Dinic {
    fn new(n: usize) -> Self {
        Self {
            n,
            g: (0..n).map(|_| Vec::new()).collect(),
        }
    }

    fn add_edge(&mut self, from: usize, to: usize, cap: i64) {
        let to_len = self.g[to].len();
        let from_len = self.g[from].len();
        self.g[from].push(Edge { to, rev: to_len, cap });
        self.g[to].push(Edge {
            to: from,
            rev: from_len,
            cap: 0,
        });
    }

    fn bfs(&self, s: usize, t: usize, level: &mut [i32]) -> bool {
        level.fill(-1);
        let mut q = std::collections::VecDeque::new();
        level[s] = 0;
        q.push_back(s);
        while let Some(v) = q.pop_front() {
            for e in &self.g[v] {
                if e.cap > 0 && level[e.to] < 0 {
                    level[e.to] = level[v] + 1;
                    q.push_back(e.to);
                }
            }
        }
        level[t] >= 0
    }

    fn dfs(
        &mut self,
        v: usize,
        t: usize,
        f: i64,
        level: &mut [i32],
        it: &mut [usize],
    ) -> i64 {
        if v == t {
            return f;
        }
        for i in it[v]..self.g[v].len() {
            it[v] = i;
            let e = self.g[v][i].to;
            let cap = self.g[v][i].cap;
            if cap > 0 && level[v] < level[e] {
                let d = self.dfs(e, t, f.min(cap), level, it);
                if d > 0 {
                    self.g[v][i].cap -= d;
                    let rev = self.g[v][i].rev;
                    self.g[e][rev].cap += d;
                    return d;
                }
            }
        }
        0
    }

    fn max_flow(&mut self, s: usize, t: usize) -> i64 {
        let mut flow = 0i64;
        let mut level = vec![0i32; self.n];
        let mut it = vec![0usize; self.n];
        while self.bfs(s, t, &mut level) {
            it.fill(0);
            loop {
                let f = self.dfs(s, t, i64::MAX, &mut level, &mut it);
                if f == 0 {
                    break;
                }
                flow += f;
            }
        }
        flow
    }
}

/// `classes[i]` = liste des niveaux présents dans la classe i (1 ou 2 niveaux).
/// Pour toute classe à **double niveau**, au moins `min_per_level_in_dual_class` élèves de **chaque** niveau y sont réservés avant le flot résiduel.
pub fn feasible_assignment(
    classes: &[Vec<LevelId>],
    level_counts: &HashMap<LevelId, u32>,
    max_per_class: u32,
    min_per_class: u32,
    min_per_level_in_dual_class: u32,
) -> Option<Assignment> {
    let k = classes.len();
    let levels: Vec<LevelId> = level_counts.keys().cloned().collect();
    let l = levels.len();

    let mut rem: HashMap<LevelId, u32> = level_counts.clone();
    let mut per_class: Vec<HashMap<LevelId, u32>> = vec![HashMap::new(); k];
    let mut cap_left: Vec<u32> = vec![max_per_class; k];

    let mdl = min_per_level_in_dual_class.max(1);
    for (ci, comp) in classes.iter().enumerate() {
        if comp.len() == 2 {
            let a = &comp[0];
            let b = &comp[1];
            let ra = *rem.get(a)?;
            let rb = *rem.get(b)?;
            let need_cap = mdl.saturating_mul(2);
            if ra < mdl || rb < mdl || cap_left[ci] < need_cap {
                return None;
            }
            *rem.get_mut(a).unwrap() -= mdl;
            *rem.get_mut(b).unwrap() -= mdl;
            cap_left[ci] -= need_cap;
            per_class[ci].insert(a.clone(), mdl);
            per_class[ci].insert(b.clone(), mdl);
        }
    }

    let s = 0;
    let level_start = 1;
    let class_start = 1 + l;
    let sink = class_start + k;
    let n = sink + 1;
    let mut dinic = Dinic::new(n);
    let mut need: i64 = 0;
    for (li, lvl) in levels.iter().enumerate() {
        let c = *rem.get(lvl).unwrap() as i64;
        need += c;
        if c > 0 {
            dinic.add_edge(s, level_start + li, c);
        }
    }
    for (ci, comp) in classes.iter().enumerate() {
        let cn = class_start + ci;
        let cap = cap_left[ci] as i64;
        if cap < 0 {
            return None;
        }
        dinic.add_edge(cn, sink, cap);
        for lvl in comp {
            let li = levels.iter().position(|x| x == lvl)?;
            dinic.add_edge(level_start + li, cn, cap);
        }
    }
    let flow = dinic.max_flow(s, sink);
    if flow != need {
        return None;
    }
    // Flux résiduel (second flot) à ajouter aux affectations déjà fixées pour les doubles niveaux.
    #[allow(clippy::needless_range_loop)]
    for ci in 0..k {
        let cn = class_start + ci;
        for e in &dinic.g[cn] {
            if e.to >= level_start && e.to < level_start + l {
                let sent = e.cap;
                if sent > 0 {
                    let idx = e.to - level_start;
                    let lvl = levels[idx].clone();
                    *per_class[ci].entry(lvl).or_insert(0) += sent as u32;
                }
            }
        }
    }
    let class_totals: Vec<u32> = per_class
        .iter()
        .map(|m| m.values().sum())
        .collect();
    if min_per_class > 0 {
        if class_totals.iter().any(|&t| t < min_per_class && t > 0) {
            // des classes vides sont permises si min=0 ; si min>0, toute classe utilisée doit respecter min
        }
        // Si une classe est vide, total 0 — on autorise les classes vides seulement si besoin?
        // Contrainte: chaque classe doit avoir soit 0 soit >= min. Le modèle actuel peut laisser une classe vide.
        for &t in &class_totals {
            if t > 0 && t < min_per_class {
                return None;
            }
        }
    }
    // Garde-fou : chaque niveau d’un double niveau a bien au moins `mdl` élèves (le flot ne peut pas en retirer).
    for (ci, comp) in classes.iter().enumerate() {
        if comp.len() == 2 {
            for lvl in comp {
                let n = *per_class[ci].get(lvl).unwrap_or(&0);
                if n < mdl {
                    return None;
                }
            }
        }
    }
    Some(Assignment {
        per_class,
        class_totals,
    })
}
