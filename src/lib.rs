//! Répartition d’élèves par niveau dans des classes (mono ou double niveau),
//! avec recherche de plusieurs plans et score pédagogique configurable.

pub mod config;
pub mod flow;
pub mod search;

pub use config::{AppConfig, LevelData, LevelId, PlanConfig};
pub use flow::{feasible_assignment, Assignment};
pub use search::{search_plans, ClassComposition, ClassRoster, Plan, PlanMetrics};
