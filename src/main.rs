use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use repartition_classes::config::AppConfig;
use repartition_classes::search::{search_plans, Plan};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "repartition-classes")]
#[command(about = "Propose plusieurs plans de répartition d’élèves en classes (mono / double niveau).")]
struct Cli {
    /// Fichier TOML de configuration (effectifs, contraintes, scoring).
    #[arg(short, long)]
    input: PathBuf,

    /// Format de sortie.
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    format: OutputFormat,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum OutputFormat {
    Text,
    Markdown,
    Json,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = AppConfig::load_path(&cli.input)
        .with_context(|| format!("impossible de charger {:?}", cli.input))?;
    eprintln!(
        "repartition-classes — min_students_per_level_in_dual_class = {} (minimum par niveau dans chaque classe à double niveau)",
        cfg.plan.min_students_per_level_in_dual_class
    );
    let plans = search_plans(&cfg);
    match cli.format {
        OutputFormat::Text => print_text(&plans),
        OutputFormat::Markdown => print_markdown(&plans),
        OutputFormat::Json => {
            let s = serde_json::to_string_pretty(&plans).context("sérialisation JSON")?;
            println!("{}", s);
        }
    }
    Ok(())
}

fn print_text(plans: &[Plan]) {
    if plans.is_empty() {
        println!("Aucun plan trouvé. Vérifiez les effectifs, num_classes, max_students_per_class et les paires interdites.");
        return;
    }
    println!("{} plan(s) proposé(s) (tri du meilleur score au moins bon)\n", plans.len());
    for (i, p) in plans.iter().enumerate() {
        println!("═══ Plan {} — score global {:.2} ═══", i + 1, p.metrics.global_score);
        println!(
            "  Double niveaux : {} — paires : {:?}",
            p.metrics.num_double_level_classes, p.metrics.double_level_pairs
        );
        println!(
            "  Effectifs : moyenne {:.1}, écart-type {:.2}, écart max à la moyenne {:.1}",
            p.metrics.mean_class_size,
            p.metrics.stdev_class_size,
            p.metrics.max_deviation_from_mean
        );
        println!();
        println!("  {:<6} | {:<28} | {:>5} | détail", "Classe", "Niveau(x)", "Total");
        println!("  {}", "-".repeat(60));
        for (ci, comp) in p.classes.iter().enumerate() {
            let label = if comp.len() == 2 {
                format!("{} + {}", comp[0], comp[1])
            } else {
                comp[0].clone()
            };
            let total = p.metrics.class_totals.get(ci).copied().unwrap_or(0);
            let mut detail = String::new();
            if let Some(m) = p.assignment.per_class.get(ci) {
                let mut parts: Vec<_> = m.iter().collect();
                parts.sort_by(|a, b| a.0.cmp(b.0));
                for (lvl, n) in parts {
                    if !detail.is_empty() {
                        detail.push_str(", ");
                    }
                    detail.push_str(&format!("{}:{}", lvl, n));
                }
            }
            println!(
                "  {:<6} | {:<28} | {:>5} | {}",
                ci + 1,
                label,
                total,
                detail
            );
            if let Some(ref rosters) = p.students_per_class {
                if let Some(map) = rosters.get(ci) {
                    if !map.is_empty() {
                        let mut keys: Vec<_> = map.keys().collect();
                        keys.sort();
                        for lvl in keys {
                            let names = &map[lvl];
                            println!(
                                "         └─ {} : {}",
                                lvl,
                                names.join(", ")
                            );
                        }
                    }
                }
            }
        }
        println!();
    }
}

fn print_markdown(plans: &[Plan]) {
    if plans.is_empty() {
        println!("*Aucun plan trouvé.*");
        return;
    }
    println!("# Plans de répartition ({})\n", plans.len());
    for (i, p) in plans.iter().enumerate() {
        println!("## Plan {} — score {:.2}\n", i + 1, p.metrics.global_score);
        println!(
            "- **Double niveaux** : {} — paires : `{:?}`",
            p.metrics.num_double_level_classes, p.metrics.double_level_pairs
        );
        println!(
            "- **Effectifs** : moyenne {:.1}, σ {:.2}, écart max {:.1}\n",
            p.metrics.mean_class_size,
            p.metrics.stdev_class_size,
            p.metrics.max_deviation_from_mean
        );
        println!("| Classe | Niveau(x) | Total | Détail |");
        println!("|--------|-----------|------:|--------|");
        for (ci, comp) in p.classes.iter().enumerate() {
            let label = if comp.len() == 2 {
                format!("{} + {}", comp[0], comp[1])
            } else {
                comp[0].clone()
            };
            let total = p.metrics.class_totals.get(ci).copied().unwrap_or(0);
            let mut detail = String::new();
            if let Some(m) = p.assignment.per_class.get(ci) {
                let mut parts: Vec<_> = m.iter().collect();
                parts.sort_by(|a, b| a.0.cmp(b.0));
                for (lvl, n) in parts {
                    if !detail.is_empty() {
                        detail.push_str(", ");
                    }
                    detail.push_str(&format!("{}:{}", lvl, n));
                }
            }
            println!("| {} | {} | {} | {} |", ci + 1, label, total, detail);
            if let Some(ref rosters) = p.students_per_class {
                if let Some(map) = rosters.get(ci) {
                    if !map.is_empty() {
                        let mut keys: Vec<_> = map.keys().collect();
                        keys.sort();
                        for lvl in keys {
                            let names = &map[lvl];
                            println!(
                                "  - **{}** : {}",
                                lvl,
                                names.join(", ")
                            );
                        }
                    }
                }
            }
        }
        println!();
    }
}
