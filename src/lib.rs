use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Core types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConservationLaw {
    pub name: String,
    pub input_sum: f64,
    pub output_sum: f64,
    pub lost: f64,
    pub created: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConservationCheck {
    pub law: ConservationLaw,
    pub balance: f64,
    pub within_tolerance: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConservationReport {
    pub checks: Vec<ConservationCheck>,
    pub total_balance: f64,
    pub violations: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileSummary {
    pub count: usize,
    pub total_value: f64,
    pub avg_confidence: f64,
    pub types: HashMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileFlow {
    pub input_tiles: Vec<TileSummary>,
    pub output_tiles: Vec<TileSummary>,
    pub layer: String,
}

// ── Default tolerance ───────────────────────────────────────────────────────

const DEFAULT_TOLERANCE: f64 = 1e-9;

// ── TileSummary ─────────────────────────────────────────────────────────────

impl TileSummary {
    pub fn from_values(values: &[(String, f64, f64)]) -> Self {
        let mut types: HashMap<String, usize> = HashMap::new();
        let mut total_value = 0.0_f64;
        let mut total_confidence = 0.0_f64;

        for (tile_type, value, confidence) in values {
            *types.entry(tile_type.clone()).or_insert(0) += 1;
            total_value += value;
            total_confidence += confidence;
        }

        let count = values.len();
        TileSummary {
            count,
            total_value,
            avg_confidence: if count > 0 { total_confidence / count as f64 } else { 0.0 },
            types,
        }
    }
}

// ── Individual checks ───────────────────────────────────────────────────────

pub fn check_tile_count(input: usize, resolved: usize, escalated: usize) -> ConservationCheck {
    let output_sum = (resolved + escalated) as f64;
    let input_sum = input as f64;
    let balance = input_sum - output_sum;
    ConservationCheck {
        law: ConservationLaw {
            name: "tile_count".into(),
            input_sum,
            output_sum,
            lost: if balance > 0.0 { balance } else { 0.0 },
            created: if balance < 0.0 { -balance } else { 0.0 },
        },
        balance,
        within_tolerance: balance.abs() < DEFAULT_TOLERANCE,
    }
}

pub fn check_information(input: &[f64], output: &[f64]) -> ConservationCheck {
    let input_sum: f64 = input.iter().copied().sum();
    let output_sum: f64 = output.iter().copied().sum();
    let balance = input_sum - output_sum;
    ConservationCheck {
        law: ConservationLaw {
            name: "information".into(),
            input_sum,
            output_sum,
            lost: if balance > 0.0 { balance } else { 0.0 },
            created: if balance < 0.0 { -balance } else { 0.0 },
        },
        balance,
        within_tolerance: balance.abs() < DEFAULT_TOLERANCE,
    }
}

pub fn check_type_preservation(input_types: &[String], output_types: &[String]) -> ConservationCheck {
    let mut input_map: HashMap<String, usize> = HashMap::new();
    let mut output_map: HashMap<String, usize> = HashMap::new();

    for t in input_types {
        *input_map.entry(t.clone()).or_insert(0) += 1;
    }
    for t in output_types {
        *output_map.entry(t.clone()).or_insert(0) += 1;
    }

    let input_sum = input_types.len() as f64;
    let output_sum = output_types.len() as f64;

    // Check: every input type must appear in output at least once
    let all_preserved = input_map.keys().all(|k| output_map.contains_key(k));

    ConservationCheck {
        law: ConservationLaw {
            name: "type_preservation".into(),
            input_sum,
            output_sum,
            lost: 0.0,
            created: 0.0,
        },
        balance: if all_preserved { 0.0 } else { 1.0 },
        within_tolerance: all_preserved,
    }
}

pub fn check_value_conservation(input: &[f64], output: &[f64], tolerance: f64) -> ConservationCheck {
    let input_sum: f64 = input.iter().copied().sum();
    let output_sum: f64 = output.iter().copied().sum();
    let balance = input_sum - output_sum;
    ConservationCheck {
        law: ConservationLaw {
            name: "value_conservation".into(),
            input_sum,
            output_sum,
            lost: if balance > 0.0 { balance } else { 0.0 },
            created: if balance < 0.0 { -balance } else { 0.0 },
        },
        balance,
        within_tolerance: balance.abs() <= tolerance,
    }
}

// ── TileFlow ────────────────────────────────────────────────────────────────

impl TileFlow {
    pub fn check_conservation(&self) -> ConservationReport {
        let mut checks = Vec::new();

        // Aggregate input/output
        let input_count: usize = self.input_tiles.iter().map(|s| s.count).sum();
        let output_count: usize = self.output_tiles.iter().map(|s| s.count).sum();

        // 1. Tile count
        checks.push(check_tile_count(input_count, output_count, 0));

        // 2. Information (confidence sums)
        let input_conf: Vec<f64> = self.input_tiles.iter()
            .flat_map(|s| std::iter::repeat(s.avg_confidence).take(s.count))
            .collect();
        let output_conf: Vec<f64> = self.output_tiles.iter()
            .flat_map(|s| std::iter::repeat(s.avg_confidence).take(s.count))
            .collect();
        checks.push(check_information(&input_conf, &output_conf));

        // 3. Type preservation
        let input_types: Vec<String> = self.input_tiles.iter()
            .flat_map(|s| s.types.keys().cloned())
            .collect();
        let output_types: Vec<String> = self.output_tiles.iter()
            .flat_map(|s| s.types.keys().cloned())
            .collect();
        checks.push(check_type_preservation(&input_types, &output_types));

        // 4. Value conservation
        let input_vals: Vec<f64> = self.input_tiles.iter()
            .flat_map(|s| {
                let avg = if s.count > 0 { s.total_value / s.count as f64 } else { 0.0 };
                std::iter::repeat(avg).take(s.count)
            })
            .collect();
        let output_vals: Vec<f64> = self.output_tiles.iter()
            .flat_map(|s| {
                let avg = if s.count > 0 { s.total_value / s.count as f64 } else { 0.0 };
                std::iter::repeat(avg).take(s.count)
            })
            .collect();
        checks.push(check_value_conservation(&input_vals, &output_vals, DEFAULT_TOLERANCE));

        let violations = checks.iter().filter(|c| !c.within_tolerance).count();
        let total_balance: f64 = checks.iter().map(|c| c.balance).sum();

        ConservationReport {
            checks,
            total_balance,
            violations,
        }
    }
}

// ── ConservationReport ──────────────────────────────────────────────────────

impl ConservationReport {
    pub fn summary(&self) -> String {
        let mut lines = Vec::new();
        lines.push("=== Conservation Report ===".into());
        for check in &self.checks {
            let status = if check.within_tolerance { "✓" } else { "✗" };
            lines.push(format!(
                "  {} {}: balance={:.6} (in={:.2} out={:.2} lost={:.2} created={:.2})",
                status,
                check.law.name,
                check.balance,
                check.law.input_sum,
                check.law.output_sum,
                check.law.lost,
                check.law.created,
            ));
        }
        lines.push(format!("Total balance: {:.6}", self.total_balance));
        lines.push(format!("Violations: {}", self.violations));
        lines.push(format!("Balanced: {}", self.is_balanced()));
        lines.join("\n")
    }

    pub fn is_balanced(&self) -> bool {
        self.violations == 0
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Tile count ──────────────────────────────────────────────────────────

    #[test]
    fn tile_count_balanced() {
        let check = check_tile_count(10, 7, 3);
        assert!(check.within_tolerance);
        assert_eq!(check.balance, 0.0);
    }

    #[test]
    fn tile_count_missing() {
        let check = check_tile_count(10, 5, 3);
        assert!(!check.within_tolerance);
        assert!(check.balance > 0.0);
        assert!(check.law.lost > 0.0);
    }

    #[test]
    fn tile_count_extra() {
        let check = check_tile_count(10, 8, 5);
        assert!(!check.within_tolerance);
        assert!(check.balance < 0.0);
        assert!(check.law.created > 0.0);
    }

    // ── Information ─────────────────────────────────────────────────────────

    #[test]
    fn information_conserved() {
        let input = vec![0.9, 0.8, 0.7];
        let output = vec![0.9, 0.8, 0.7];
        let check = check_information(&input, &output);
        assert!(check.within_tolerance);
    }

    #[test]
    fn information_lost() {
        let input = vec![0.9, 0.8, 0.7];
        let output = vec![0.5, 0.4];
        let check = check_information(&input, &output);
        assert!(!check.within_tolerance);
        assert!(check.law.lost > 0.0);
    }

    #[test]
    fn information_created() {
        let input = vec![0.5];
        let output = vec![0.9, 0.8];
        let check = check_information(&input, &output);
        assert!(!check.within_tolerance);
        assert!(check.law.created > 0.0);
    }

    // ── Type preservation ───────────────────────────────────────────────────

    #[test]
    fn type_preservation_all_present() {
        let input = vec!["hot".into(), "cold".into(), "warm".into()];
        let output = vec!["hot".into(), "cold".into(), "warm".into()];
        let check = check_type_preservation(&input, &output);
        assert!(check.within_tolerance);
    }

    #[test]
    fn type_preservation_missing_type() {
        let input = vec!["hot".into(), "cold".into(), "warm".into()];
        let output = vec!["hot".into(), "cold".into()];
        let check = check_type_preservation(&input, &output);
        assert!(!check.within_tolerance);
    }

    #[test]
    fn type_preservation_extra_types_ok() {
        let input = vec!["hot".into()];
        let output = vec!["hot".into(), "cold".into()];
        let check = check_type_preservation(&input, &output);
        assert!(check.within_tolerance);
    }

    // ── Value conservation ──────────────────────────────────────────────────

    #[test]
    fn value_conserved_within_tolerance() {
        let input = vec![10.0, 20.0, 30.0];
        let output = vec![15.0, 25.0, 20.0];
        let check = check_value_conservation(&input, &output, 1e-6);
        assert!(check.within_tolerance);
    }

    #[test]
    fn value_not_conserved() {
        let input = vec![10.0, 20.0];
        let output = vec![5.0, 10.0];
        let check = check_value_conservation(&input, &output, 1e-6);
        assert!(!check.within_tolerance);
    }

    #[test]
    fn value_tolerance_sensitivity() {
        let input = vec![10.0];
        let output = vec![10.0001];
        let strict = check_value_conservation(&input, &output, 1e-6);
        let loose = check_value_conservation(&input, &output, 0.01);
        assert!(!strict.within_tolerance);
        assert!(loose.within_tolerance);
    }

    // ── TileFlow full report ────────────────────────────────────────────────

    #[test]
    fn tileflow_balanced_report() {
        let flow = TileFlow {
            input_tiles: vec![TileSummary::from_values(&[
                ("hot".into(), 10.0, 0.9),
                ("cold".into(), 20.0, 0.8),
            ])],
            output_tiles: vec![TileSummary::from_values(&[
                ("hot".into(), 10.0, 0.9),
                ("cold".into(), 20.0, 0.8),
            ])],
            layer: "detection".into(),
        };
        let report = flow.check_conservation();
        assert!(report.is_balanced());
        assert_eq!(report.violations, 0);
    }

    #[test]
    fn tileflow_unbalanced_report() {
        let flow = TileFlow {
            input_tiles: vec![TileSummary::from_values(&[
                ("hot".into(), 10.0, 0.9),
            ])],
            output_tiles: vec![TileSummary::from_values(&[]),
            ],
            layer: "detection".into(),
        };
        let report = flow.check_conservation();
        assert!(!report.is_balanced());
        assert!(report.violations > 0);
    }

    #[test]
    fn report_summary_readable() {
        let flow = TileFlow {
            input_tiles: vec![TileSummary::from_values(&[
                ("hot".into(), 10.0, 0.9),
            ])],
            output_tiles: vec![TileSummary::from_values(&[
                ("hot".into(), 10.0, 0.9),
            ])],
            layer: "test".into(),
        };
        let report = flow.check_conservation();
        let summary = report.summary();
        assert!(summary.contains("Conservation Report"));
        assert!(summary.contains("tile_count"));
    }

    // ── Edge cases ──────────────────────────────────────────────────────────

    #[test]
    fn zero_input() {
        let check = check_tile_count(0, 0, 0);
        assert!(check.within_tolerance);
    }

    #[test]
    fn single_tile() {
        let check = check_information(&[0.5], &[0.5]);
        assert!(check.within_tolerance);
    }

    #[test]
    fn all_resolved() {
        let check = check_tile_count(5, 5, 0);
        assert!(check.within_tolerance);
    }

    #[test]
    fn all_escalated() {
        let check = check_tile_count(5, 0, 5);
        assert!(check.within_tolerance);
    }

    #[test]
    fn multiple_laws_simultaneously() {
        let flow = TileFlow {
            input_tiles: vec![TileSummary::from_values(&[
                ("A".into(), 1.0, 0.5),
                ("B".into(), 2.0, 0.6),
                ("C".into(), 3.0, 0.7),
            ])],
            output_tiles: vec![TileSummary::from_values(&[
                ("A".into(), 1.0, 0.5),
                ("B".into(), 2.0, 0.6),
                ("C".into(), 3.0, 0.7),
            ])],
            layer: "chain".into(),
        };
        let report = flow.check_conservation();
        assert_eq!(report.checks.len(), 4);
        assert!(report.is_balanced());
    }

    #[test]
    fn tile_summary_from_values() {
        let summary = TileSummary::from_values(&[
            ("hot".into(), 10.0, 0.9),
            ("hot".into(), 20.0, 0.7),
            ("cold".into(), 5.0, 0.5),
        ]);
        assert_eq!(summary.count, 3);
        assert!((summary.total_value - 35.0).abs() < 1e-9);
        assert!((summary.avg_confidence - 0.7).abs() < 1e-9);
        assert_eq!(*summary.types.get("hot").unwrap(), 2);
        assert_eq!(*summary.types.get("cold").unwrap(), 1);
    }
}
