//! Threshold-based perf gates for benchmark suites.
//!
//! Gates are opt-in and intended for CI. Enable with:
//! `STATESET_PERF_GATE=1`.
//!
//! Thresholds are loaded from JSON:
//! - default: `crates/stateset-benches/perf-gates.json`
//! - override path with `STATESET_PERF_GATE_FILE=/path/to/file.json`

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

const DEFAULT_TOLERANCE_RATIO: f64 = 0.20;
const DEFAULT_ITERATIONS: u64 = 10_000;
const PERF_GATE_ENV: &str = "STATESET_PERF_GATE";
const PERF_GATE_FILE_ENV: &str = "STATESET_PERF_GATE_FILE";
const PERF_GATE_TOLERANCE_ENV: &str = "STATESET_PERF_GATE_TOLERANCE";
const PERF_GATE_ITERS_ENV: &str = "STATESET_PERF_GATE_ITERATIONS";

#[derive(Debug, Clone)]
struct PerfGateThresholds {
    tolerance_ratio: f64,
    default_iterations: u64,
    max_ns_per_iter: HashMap<String, f64>,
    iterations: HashMap<String, u64>,
}

impl Default for PerfGateThresholds {
    fn default() -> Self {
        Self {
            tolerance_ratio: DEFAULT_TOLERANCE_RATIO,
            default_iterations: DEFAULT_ITERATIONS,
            max_ns_per_iter: HashMap::new(),
            iterations: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct PerfGate {
    enabled: bool,
    thresholds: PerfGateThresholds,
}

impl PerfGate {
    fn from_env() -> Self {
        let enabled = env_flag(PERF_GATE_ENV);
        if !enabled {
            return Self { enabled: false, thresholds: PerfGateThresholds::default() };
        }

        let path = std::env::var(PERF_GATE_FILE_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("crates/stateset-benches/perf-gates.json"));

        let mut thresholds = load_thresholds(&path).unwrap_or_else(|err| {
            panic!("failed to load perf gate configuration from {}: {err}", path.display())
        });

        if let Ok(raw) = std::env::var(PERF_GATE_TOLERANCE_ENV) {
            let parsed = raw.parse::<f64>().unwrap_or_else(|err| {
                panic!("invalid {PERF_GATE_TOLERANCE_ENV}={raw}: {err}");
            });
            thresholds.tolerance_ratio = parsed.max(0.0);
        }

        if let Ok(raw) = std::env::var(PERF_GATE_ITERS_ENV) {
            let parsed = raw.parse::<u64>().unwrap_or_else(|err| {
                panic!("invalid {PERF_GATE_ITERS_ENV}={raw}: {err}");
            });
            thresholds.default_iterations = parsed.max(1);
        }

        Self { enabled: true, thresholds }
    }

    fn iterations_for(&self, name: &str) -> u64 {
        self.thresholds
            .iterations
            .get(name)
            .copied()
            .unwrap_or(self.thresholds.default_iterations)
            .max(1)
    }

    fn run_if_enabled(&self, name: &str, iterations: u64, mut op: impl FnMut()) {
        if !self.enabled {
            return;
        }
        let Some(max_ns) = self.thresholds.max_ns_per_iter.get(name).copied() else {
            return;
        };

        let iterations = iterations.max(1);
        let started = Instant::now();
        for _ in 0..iterations {
            op();
        }
        let elapsed = started.elapsed();
        self.assert_within_budget(name, elapsed, iterations, max_ns);
    }

    fn assert_within_budget(&self, name: &str, elapsed: Duration, iterations: u64, max_ns: f64) {
        let observed_ns_per_iter = elapsed.as_nanos() as f64 / iterations as f64;
        let allowed = max_ns * (1.0 + self.thresholds.tolerance_ratio.max(0.0));
        assert!(
            observed_ns_per_iter <= allowed,
            "perf gate failed for '{name}': observed {:.2}ns/iter > allowed {:.2}ns/iter \
             (baseline {:.2}, tolerance {:.2}%)",
            observed_ns_per_iter,
            allowed,
            max_ns,
            self.thresholds.tolerance_ratio * 100.0
        );
    }
}

fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .map(|v| matches!(v.trim(), "1" | "true" | "TRUE" | "yes" | "on"))
        .unwrap_or(false)
}

fn parse_f64_map(value: &serde_json::Value, field: &str) -> Result<HashMap<String, f64>, String> {
    let Some(map) = value.get(field) else {
        return Ok(HashMap::new());
    };
    let obj = map.as_object().ok_or_else(|| format!("'{field}' must be an object"))?;
    let mut out = HashMap::with_capacity(obj.len());
    for (k, v) in obj {
        let n =
            v.as_f64().ok_or_else(|| format!("'{field}.{k}' must be a number (ns per iter)"))?;
        out.insert(k.clone(), n);
    }
    Ok(out)
}

fn parse_u64_map(value: &serde_json::Value, field: &str) -> Result<HashMap<String, u64>, String> {
    let Some(map) = value.get(field) else {
        return Ok(HashMap::new());
    };
    let obj = map.as_object().ok_or_else(|| format!("'{field}' must be an object"))?;
    let mut out = HashMap::with_capacity(obj.len());
    for (k, v) in obj {
        let n = v.as_u64().ok_or_else(|| format!("'{field}.{k}' must be an unsigned integer"))?;
        out.insert(k.clone(), n.max(1));
    }
    Ok(out)
}

fn load_thresholds(path: &std::path::Path) -> Result<PerfGateThresholds, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|err| format!("unable to read {}: {err}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|err| format!("invalid JSON in {}: {err}", path.display()))?;
    let obj = value
        .as_object()
        .ok_or_else(|| format!("{} must contain a top-level object", path.display()))?;

    let tolerance_ratio = obj
        .get("tolerance_ratio")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(DEFAULT_TOLERANCE_RATIO);
    let default_iterations = obj
        .get("default_iterations")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(DEFAULT_ITERATIONS)
        .max(1);

    let max_ns_per_iter = parse_f64_map(&value, "max_ns_per_iter")?;
    let iterations = parse_u64_map(&value, "iterations")?;

    Ok(PerfGateThresholds {
        tolerance_ratio: tolerance_ratio.max(0.0),
        default_iterations,
        max_ns_per_iter,
        iterations,
    })
}

static PERF_GATE: OnceLock<PerfGate> = OnceLock::new();

fn global_perf_gate() -> &'static PerfGate {
    PERF_GATE.get_or_init(PerfGate::from_env)
}

/// Returns whether perf gates are enabled for this process.
#[must_use]
pub fn perf_gate_enabled() -> bool {
    global_perf_gate().enabled
}

/// Run a threshold gate using the configured default iterations.
pub fn run_gate_if_enabled(name: &str, op: impl FnMut()) {
    let gate = global_perf_gate();
    gate.run_if_enabled(name, gate.iterations_for(name), op);
}

/// Run a threshold gate using explicit iterations.
pub fn run_gate_if_enabled_with_iterations(name: &str, iterations: u64, op: impl FnMut()) {
    global_perf_gate().run_if_enabled(name, iterations, op);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_gate(enabled: bool, tolerance_ratio: f64, max_ns_per_iter: f64) -> PerfGate {
        let mut thresholds = PerfGateThresholds {
            tolerance_ratio,
            default_iterations: 10,
            max_ns_per_iter: HashMap::new(),
            iterations: HashMap::new(),
        };
        thresholds.max_ns_per_iter.insert("bench".to_string(), max_ns_per_iter);
        PerfGate { enabled, thresholds }
    }

    #[test]
    fn parse_thresholds_from_json() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("perf-gates.json");
        std::fs::write(
            &path,
            r#"{
                "tolerance_ratio": 0.10,
                "default_iterations": 1000,
                "max_ns_per_iter": {
                    "money_add": 500.0
                },
                "iterations": {
                    "money_add": 2000
                }
            }"#,
        )
        .unwrap();

        let thresholds = load_thresholds(&path).unwrap();
        assert_eq!(thresholds.default_iterations, 1000);
        assert_eq!(thresholds.tolerance_ratio, 0.10);
        assert_eq!(thresholds.max_ns_per_iter.get("money_add"), Some(&500.0));
        assert_eq!(thresholds.iterations.get("money_add"), Some(&2000));
    }

    #[test]
    fn gate_allows_values_within_tolerance() {
        let gate = test_gate(true, 0.10, 1000.0);
        gate.assert_within_budget("bench", Duration::from_nanos(1_000), 1, 1000.0);
    }

    #[test]
    fn gate_panics_when_budget_exceeded() {
        let gate = test_gate(true, 0.0, 1000.0);
        let result = std::panic::catch_unwind(|| {
            gate.assert_within_budget("bench", Duration::from_nanos(2_000), 1, 1000.0);
        });
        assert!(result.is_err());
    }

    #[test]
    fn disabled_gate_skips_execution() {
        let gate = test_gate(false, 0.0, 1.0);
        let mut called = false;
        gate.run_if_enabled("bench", 1, || {
            called = true;
        });
        assert!(!called);
    }
}
