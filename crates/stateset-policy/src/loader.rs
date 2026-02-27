use crate::policy_set::PolicySet;

/// Load a policy set from a JSON string.
///
/// # Errors
///
/// Returns [`PolicyError::JsonError`](crate::PolicyError::JsonError) if parsing fails.
///
/// # Examples
///
/// ```
/// use stateset_policy::load_policy_set_from_json;
///
/// let json = r#"{
///     "name": "test-policy",
///     "domain": "orders",
///     "rules": []
/// }"#;
///
/// let ps = load_policy_set_from_json(json).unwrap();
/// assert_eq!(ps.name, "test-policy");
/// assert_eq!(ps.domain, "orders");
/// ```
pub fn load_policy_set_from_json(content: &str) -> crate::Result<PolicySet> {
    let ps: PolicySet = serde_json::from_str(content)?;
    Ok(ps)
}

/// Load a policy set from a YAML string.
///
/// # Errors
///
/// Returns [`PolicyError::YamlError`](crate::PolicyError::YamlError) if parsing fails.
#[cfg(feature = "yaml")]
pub fn load_policy_set_from_yaml(content: &str) -> crate::Result<PolicySet> {
    let ps: PolicySet =
        serde_yaml::from_str(content).map_err(|e| crate::PolicyError::YamlError(e.to_string()))?;
    Ok(ps)
}

/// Load all policy files from a directory in strict mode.
///
/// Scans for files with extensions `.yaml`, `.yml`, and `.json`.
/// Any read or parse failure returns an error.
///
/// # Errors
///
/// Returns [`PolicyError::IoError`](crate::PolicyError::IoError) if the directory cannot be read
/// and a parse/serialization error if a policy file is invalid.
#[cfg(feature = "yaml")]
pub fn load_policies_from_dir(dir: &std::path::Path) -> crate::Result<Vec<PolicySet>> {
    load_policies_from_dir_inner(dir, false)
}

/// Load all policy files from a directory in permissive mode.
///
/// Scans for files with extensions `.yaml`, `.yml`, and `.json`.
/// Files that fail to parse are logged and skipped.
///
/// # Errors
///
/// Returns [`PolicyError::IoError`](crate::PolicyError::IoError) if the directory cannot be read.
#[cfg(feature = "yaml")]
pub fn load_policies_from_dir_permissive(dir: &std::path::Path) -> crate::Result<Vec<PolicySet>> {
    load_policies_from_dir_inner(dir, true)
}

#[cfg(feature = "yaml")]
fn load_policies_from_dir_inner(
    dir: &std::path::Path,
    permissive: bool,
) -> crate::Result<Vec<PolicySet>> {
    use std::fs;

    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut sets = Vec::new();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        if !matches!(ext, "yaml" | "yml" | "json") {
            continue;
        }

        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                if permissive {
                    tracing::warn!(
                        path = %path.display(),
                        error = %e,
                        "Failed to read policy file, skipping"
                    );
                    continue;
                }
                return Err(e.into());
            }
        };

        let result = if ext == "json" {
            load_policy_set_from_json(&content)
        } else {
            load_policy_set_from_yaml(&content)
        };

        match result {
            Ok(ps) => {
                tracing::info!(
                    name = %ps.name,
                    domain = %ps.domain,
                    rules = ps.rules.len(),
                    "Loaded policy set"
                );
                sets.push(ps);
            }
            Err(e) => {
                if permissive {
                    tracing::warn!(
                        path = %path.display(),
                        error = %e,
                        "Failed to parse policy file, skipping"
                    );
                    continue;
                }
                return Err(e);
            }
        }
    }

    Ok(sets)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_json_minimal() {
        let json = r#"{
            "name": "test",
            "domain": "orders",
            "rules": []
        }"#;
        let ps = load_policy_set_from_json(json).unwrap();
        assert_eq!(ps.name, "test");
        assert_eq!(ps.domain, "orders");
        assert!(ps.rules.is_empty());
    }

    #[test]
    fn load_json_with_rules() {
        let json = r#"{
            "name": "order-limits",
            "domain": "orders",
            "rules": [
                {
                    "name": "high-value",
                    "description": "Flag high-value orders",
                    "priority": 100,
                    "conditions": {
                        "logic": "and",
                        "conditions": [
                            {
                                "field": "order.total",
                                "operator": "gt",
                                "value": 1000
                            }
                        ]
                    },
                    "action": {
                        "type": "deny",
                        "reason": "Exceeds limit"
                    }
                }
            ]
        }"#;
        let ps = load_policy_set_from_json(json).unwrap();
        assert_eq!(ps.rules.len(), 1);
        assert_eq!(ps.rules[0].name, "high-value");
        assert_eq!(ps.rules[0].priority, 100);
    }

    #[test]
    fn load_json_sorts_rules_by_priority() {
        let json = r#"{
            "name": "priority-check",
            "domain": "orders",
            "rules": [
                {
                    "name": "low-priority",
                    "description": "low",
                    "priority": 10,
                    "conditions": { "logic": "and", "conditions": [] },
                    "action": { "type": "allow" }
                },
                {
                    "name": "high-priority",
                    "description": "high",
                    "priority": 100,
                    "conditions": { "logic": "and", "conditions": [] },
                    "action": { "type": "deny", "reason": "nope" }
                }
            ]
        }"#;

        let ps = load_policy_set_from_json(json).unwrap();
        assert_eq!(ps.rules.len(), 2);
        assert_eq!(ps.rules[0].name, "high-priority");
        assert_eq!(ps.rules[1].name, "low-priority");
    }

    #[test]
    fn load_json_invalid() {
        let result = load_policy_set_from_json("not json");
        assert!(result.is_err());
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn load_yaml_minimal() {
        let yaml = r#"
name: test-yaml
domain: returns
rules: []
"#;
        let ps = load_policy_set_from_yaml(yaml).unwrap();
        assert_eq!(ps.name, "test-yaml");
        assert_eq!(ps.domain, "returns");
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn load_yaml_with_rules() {
        let yaml = r#"
name: return-policy
domain: returns
rules:
  - name: auto-approve
    description: Auto-approve small returns
    priority: 100
    conditions:
      logic: and
      conditions:
        - field: return.value
          operator: lt
          value: 100
    action:
      type: allow
    stopOnMatch: true
defaultAction:
  type: allow
"#;
        let ps = load_policy_set_from_yaml(yaml).unwrap();
        assert_eq!(ps.rules.len(), 1);
        assert_eq!(ps.rules[0].name, "auto-approve");
        assert!(ps.rules[0].stop_on_match);
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn load_yaml_sorts_rules_by_priority() {
        let yaml = r#"
name: return-policy
domain: returns
rules:
  - name: low-priority
    description: low
    priority: 10
    conditions:
      logic: and
      conditions: []
    action:
      type: allow
  - name: high-priority
    description: high
    priority: 100
    conditions:
      logic: and
      conditions: []
    action:
      type: deny
      reason: blocked
"#;

        let ps = load_policy_set_from_yaml(yaml).unwrap();
        assert_eq!(ps.rules.len(), 2);
        assert_eq!(ps.rules[0].name, "high-priority");
        assert_eq!(ps.rules[1].name, "low-priority");
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn load_yaml_invalid() {
        let result = load_policy_set_from_yaml(":\n  bad:\n    - [unclosed");
        assert!(result.is_err());
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn load_from_dir_mixed_files() {
        use std::fs;
        use std::io::Write as _;

        let dir = tempfile::tempdir().unwrap();

        // Write a JSON policy
        let json_path = dir.path().join("orders.json");
        let mut f = fs::File::create(&json_path).unwrap();
        writeln!(f, r#"{{"name": "json-policy", "domain": "orders", "rules": []}}"#).unwrap();

        // Write a YAML policy
        let yaml_path = dir.path().join("returns.yaml");
        let mut f = fs::File::create(&yaml_path).unwrap();
        writeln!(f, "name: yaml-policy\ndomain: returns\nrules: []").unwrap();

        // Write a non-policy file (should be ignored)
        let txt_path = dir.path().join("readme.txt");
        fs::write(&txt_path, "ignore me").unwrap();

        let sets = load_policies_from_dir(dir.path()).unwrap();
        assert_eq!(sets.len(), 2);

        let names: Vec<&str> = sets.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"json-policy"));
        assert!(names.contains(&"yaml-policy"));
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn load_from_nonexistent_dir() {
        let result = load_policies_from_dir(std::path::Path::new("/nonexistent/dir"));
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn load_from_dir_strict_rejects_bad_files() {
        use std::fs;
        use std::io::Write as _;

        let dir = tempfile::tempdir().unwrap();

        // Write a valid JSON policy
        let json_path = dir.path().join("good.json");
        let mut f = fs::File::create(&json_path).unwrap();
        writeln!(f, r#"{{"name": "good", "domain": "orders", "rules": []}}"#).unwrap();

        // Write an invalid JSON file
        let bad_path = dir.path().join("bad.json");
        fs::write(&bad_path, "not valid json").unwrap();

        let err = load_policies_from_dir(dir.path()).unwrap_err();
        assert!(!err.to_string().is_empty());
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn load_from_dir_permissive_skips_bad_files() {
        use std::fs;
        use std::io::Write as _;

        let dir = tempfile::tempdir().unwrap();

        let json_path = dir.path().join("good.json");
        let mut f = fs::File::create(&json_path).unwrap();
        writeln!(f, r#"{{"name": "good", "domain": "orders", "rules": []}}"#).unwrap();

        let bad_path = dir.path().join("bad.json");
        fs::write(&bad_path, "not valid json").unwrap();

        let sets = load_policies_from_dir_permissive(dir.path()).unwrap();
        assert_eq!(sets.len(), 1);
        assert_eq!(sets[0].name, "good");
    }
}
