use repartition_classes::config::AppConfig;
use repartition_classes::flow::feasible_assignment;
use repartition_classes::search::search_plans;
use std::collections::HashMap;
use std::io::Write;
use tempfile::NamedTempFile;

fn write_toml(content: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(content.as_bytes()).unwrap();
    f.flush().unwrap();
    f
}

#[test]
fn single_class_single_level() {
    let f = write_toml(
        r#"
num_classes = 1
max_students_per_class = 30

[levels]
CP = 24
"#,
    );
    let cfg = AppConfig::load_path(f.path()).unwrap();
    let plans = search_plans(&cfg);
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].classes[0], vec!["CP"]);
}

#[test]
fn one_teacher_two_levels_must_be_double_or_infeasible() {
    let f = write_toml(
        r#"
num_classes = 1
max_students_per_class = 40

[levels]
CP = 20
CE1 = 18
"#,
    );
    let cfg = AppConfig::load_path(f.path()).unwrap();
    let plans = search_plans(&cfg);
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].classes[0].len(), 2);
}

#[test]
fn imbalanced_requires_split_across_classes() {
    let f = write_toml(
        r#"
num_classes = 2
max_students_per_class = 12

[levels]
CP = 24
"#,
    );
    let cfg = AppConfig::load_path(f.path()).unwrap();
    let plans = search_plans(&cfg);
    assert!(!plans.is_empty());
    let appearances = plans[0]
        .classes
        .iter()
        .filter(|c| c.contains(&"CP".to_string()))
        .count();
    assert_eq!(appearances, 2);
}

#[test]
fn forbidden_pair_eliminates_dual() {
    let f = write_toml(
        r#"
num_classes = 2
max_students_per_class = 25
forbidden_pairs = [["CP", "CM2"]]

level_order = ["CP", "CE1", "CE2", "CM1", "CM2"]

[levels]
CP = 10
CM2 = 10
"#,
    );
    let cfg = AppConfig::load_path(f.path()).unwrap();
    let plans = search_plans(&cfg);
    for p in &plans {
        for c in &p.classes {
            if c.len() == 2 {
                let a = &c[0];
                let b = &c[1];
                assert!(!(a == "CP" && b == "CM2"));
                assert!(!(a == "CM2" && b == "CP"));
            }
        }
    }
}

#[test]
fn flow_infeasible_returns_none() {
    let mut counts = HashMap::new();
    counts.insert("A".into(), 30u32);
    counts.insert("B".into(), 30u32);
    let classes = vec![vec!["A".into(), "B".into()]];
    assert!(feasible_assignment(&classes, &counts, 25, 0, 1).is_none());
}

#[test]
fn dual_class_requires_at_least_one_per_level() {
    let mut counts = HashMap::new();
    counts.insert("A".into(), 5u32);
    counts.insert("B".into(), 5u32);
    let classes = vec![vec!["A".into(), "B".into()]];
    let a = feasible_assignment(&classes, &counts, 15, 0, 1).expect("faisable");
    let m = &a.per_class[0];
    assert!(*m.get("A").unwrap_or(&0) >= 1);
    assert!(*m.get("B").unwrap_or(&0) >= 1);
}

#[test]
fn named_students_list_partitioned_by_class_order() {
    let f = write_toml(
        r#"
num_classes = 2
max_students_per_class = 10
num_plans = 3
dfs_node_budget = 50000
random_trials = 5000

level_order = ["CP", "CE1"]

[levels]
CP = { students = ["Ana", "Bob", "Cid", "Dan", "Eva"] }
CE1 = 6
"#,
    );
    let cfg = AppConfig::load_path(f.path()).unwrap();
    assert!(cfg.has_named_students());
    let plans = search_plans(&cfg);
    assert!(!plans.is_empty());
    let p = &plans[0];
    let roster = p.students_per_class.as_ref().expect("roster");
    let mut cp_all: Vec<String> = Vec::new();
    for map in roster {
        if let Some(v) = map.get("CP") {
            cp_all.extend(v.clone());
        }
    }
    assert_eq!(cp_all.len(), 5);
    assert_eq!(cp_all, vec!["Ana", "Bob", "Cid", "Dan", "Eva"]);
}

#[test]
fn min_students_per_level_in_dual_rejects_imbalanced_pair() {
    let mut counts = HashMap::new();
    counts.insert("CM1".into(), 29u32);
    counts.insert("CM2".into(), 1u32);
    let classes = vec![vec!["CM1".into(), "CM2".into()]];
    assert!(feasible_assignment(&classes, &counts, 40, 0, 3).is_none());
    assert!(feasible_assignment(&classes, &counts, 40, 0, 1).is_some());
}

#[test]
fn min_students_per_level_in_dual_config_excludes_all_plans_when_only_double_possible() {
    let f = write_toml(
        r#"
num_classes = 1
max_students_per_class = 40
min_students_per_level_in_dual_class = 3
level_order = ["CM1", "CM2"]

[levels]
CM1 = 29
CM2 = 1
"#,
    );
    let cfg = AppConfig::load_path(f.path()).unwrap();
    let plans = search_plans(&cfg);
    assert!(plans.is_empty());
}

#[test]
fn duplicate_student_name_rejected() {
    let f = write_toml(
        r#"
num_classes = 1
max_students_per_class = 10
level_order = ["CP", "CE1"]

[levels]
CP = { students = ["Same", "X"] }
CE1 = { students = ["Same"] }
"#,
    );
    assert!(AppConfig::load_path(f.path()).is_err());
}

#[test]
fn flow_feasible_split() {
    let mut counts = HashMap::new();
    counts.insert("A".into(), 30u32);
    let classes = vec![vec!["A".into()], vec!["A".into()]];
    let a = feasible_assignment(&classes, &counts, 25, 0, 1);
    assert!(a.is_some());
    let a = a.unwrap();
    let s: u32 = a.class_totals.iter().sum();
    assert_eq!(s, 30);
}
