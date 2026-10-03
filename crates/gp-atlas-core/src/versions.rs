//! Client-side computations over `package version list` rows (SPEC §8, AC-24).

use std::collections::BTreeMap;

use serde_json::Value;

/// `(Major, Minor, Patch, Build)`.
pub type VersionKey = (u64, u64, u64, u64);

/// `(Major, Minor, Patch, Build)` of a version row, if all four are numbers.
pub fn version_key(row: &Value) -> Option<VersionKey> {
    let n = |k: &str| row.get(k).and_then(Value::as_u64);
    Some((
        n("MajorVersion")?,
        n("MinorVersion")?,
        n("PatchVersion")?,
        n("BuildNumber")?,
    ))
}

/// `IsReleased` is a bool in JSON mode, a string in table mode (F17).
pub fn is_released(row: &Value) -> bool {
    match row.get("IsReleased") {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => s.eq_ignore_ascii_case("true"),
        _ => false,
    }
}

/// Latest released version per `Package2Id`, by max `(Major, Minor, Patch, Build)`.
/// "Computed by GP Atlas", not by the CLI.
pub fn latest_released(rows: &[Value]) -> Vec<&Value> {
    let mut best: BTreeMap<&str, (&Value, VersionKey)> = BTreeMap::new();
    for row in rows.iter().filter(|r| is_released(r)) {
        let (Some(pkg), Some(key)) = (
            row.get("Package2Id").and_then(Value::as_str),
            version_key(row),
        ) else {
            continue;
        };
        match best.get(pkg) {
            Some((_, k)) if *k >= key => {}
            _ => {
                best.insert(pkg, (row, key));
            }
        }
    }
    best.into_values().map(|(r, _)| r).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn row(pkg: &str, v: (u64, u64, u64, u64), released: bool) -> Value {
        json!({"Package2Id": pkg, "MajorVersion": v.0, "MinorVersion": v.1,
               "PatchVersion": v.2, "BuildNumber": v.3, "IsReleased": released})
    }

    #[test]
    fn picks_max_released_per_package() {
        let rows = vec![
            row("A", (1, 2, 0, 3), true),
            row("A", (1, 10, 0, 1), true), // 10 > 2 numerically, not lexically
            row("A", (2, 0, 0, 1), false), // not released
            row("B", (0, 1, 0, 1), true),
            row("C", (5, 0, 0, 1), false),
        ];
        let got: Vec<(String, Option<VersionKey>)> = latest_released(&rows)
            .into_iter()
            .map(|r| (r["Package2Id"].as_str().unwrap().to_owned(), version_key(r)))
            .collect();
        assert_eq!(
            got,
            [
                ("A".to_owned(), Some((1, 10, 0, 1))),
                ("B".to_owned(), Some((0, 1, 0, 1)))
            ]
        );
    }

    #[test]
    fn released_as_string() {
        assert!(is_released(&json!({"IsReleased": "true"})));
        assert!(!is_released(&json!({"IsReleased": "false"})));
    }
}
