//! Parsing `sf … --json` stdout (SPEC §4.5) and scrubbing secrets (§10).

use serde_json::{Map, Value};

/// A failed command, as printed on stdout (F7).
#[derive(Debug, Clone, PartialEq)]
pub struct SfError {
    pub name: String,
    pub message: String,
    pub code: Option<String>,
    pub exit_code: Option<i64>,
    pub status: Option<i64>,
    pub context: Option<String>,
    pub command_name: Option<String>,
    pub actions: Vec<String>,
    pub warnings: Vec<String>,
}

/// What a command printed on stdout.
#[derive(Debug, Clone, PartialEq)]
pub enum SfOutput {
    /// `{"status":0,"result":…,"warnings":[…]}` (F6).
    Success {
        result: Value,
        warnings: Vec<String>,
    },
    /// Error envelope (F7).
    Error(SfError),
    /// Anything else that is valid JSON: `version` prints a bare object,
    /// `plugins` and `commands` a bare array (F3, F4).
    Bare(Value),
}

/// stdout was not JSON.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("stdout is not JSON: {0}")]
pub struct NotJson(pub String);

fn strings(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Array(a)) => a
            .iter()
            .map(|x| match x {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .collect(),
        Some(Value::String(s)) => vec![s.clone()],
        _ => Vec::new(),
    }
}

fn opt_string(o: &Map<String, Value>, key: &str) -> Option<String> {
    match o.get(key) {
        Some(Value::String(s)) => Some(s.clone()),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    }
}

/// Parses stdout. Secrets are scrubbed before anything else sees the value.
pub fn parse(stdout: &[u8]) -> Result<SfOutput, NotJson> {
    let text = String::from_utf8_lossy(stdout);
    let text = text.trim_start_matches('\u{feff}').trim();
    let mut value: Value = serde_json::from_str(text).map_err(|e| NotJson(e.to_string()))?;
    scrub(&mut value);
    Ok(classify_value(value))
}

fn classify_value(value: Value) -> SfOutput {
    let Value::Object(o) = &value else {
        return SfOutput::Bare(value);
    };
    let status = o.get("status").and_then(Value::as_i64);
    if status == Some(0) && o.contains_key("result") {
        let Value::Object(mut o) = value else {
            unreachable!()
        };
        let warnings = strings(o.get("warnings"));
        let result = o.remove("result").unwrap_or(Value::Null);
        return SfOutput::Success { result, warnings };
    }
    if status.is_some_and(|s| s != 0) && o.contains_key("name") {
        return SfOutput::Error(SfError {
            name: opt_string(o, "name").unwrap_or_default(),
            message: opt_string(o, "message").unwrap_or_default(),
            code: opt_string(o, "code"),
            exit_code: o.get("exitCode").and_then(Value::as_i64),
            status,
            context: opt_string(o, "context"),
            command_name: opt_string(o, "commandName"),
            actions: strings(o.get("actions")),
            warnings: strings(o.get("warnings")),
        });
    }
    SfOutput::Bare(value)
}

/// Field names that must never be kept (SPEC §10, F25, F34). Case-insensitive.
pub const SECRET_KEYS: &[&str] = &[
    "accesstoken",
    "password",
    "refreshtoken",
    "sfdxauthurl",
    "clientsecret",
    "privatekey",
    "installationkey",
    "installkey",
];

/// Recursively removes secret fields from a JSON value.
pub fn scrub(v: &mut Value) {
    match v {
        Value::Object(o) => {
            o.retain(|k, _| !SECRET_KEYS.contains(&k.to_ascii_lowercase().as_str()));
            o.values_mut().for_each(scrub);
        }
        Value::Array(a) => a.iter_mut().for_each(scrub),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/sf-2.150.6")
                .join(name),
        )
        .unwrap()
    }

    #[test]
    fn parses_success_envelope() {
        match parse(&fixture("org-list.real.json")).unwrap() {
            SfOutput::Success { result, warnings } => {
                assert!(result.get("devHubs").is_some());
                assert_eq!(warnings.len(), 1);
                // F25: accessToken is gone after scrubbing.
                assert!(!result.to_string().contains("accessToken"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parses_error_envelope() {
        match parse(&fixture("package-list.named-org-not-found.json")).unwrap() {
            SfOutput::Error(e) => {
                assert_eq!(e.name, "NamedOrgNotFoundError");
                assert_eq!(e.exit_code, Some(2));
                assert_eq!(e.status, Some(2));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parses_bare_outputs() {
        assert!(matches!(
            parse(&fixture("version.json")).unwrap(),
            SfOutput::Bare(Value::Object(_))
        ));
        assert!(matches!(
            parse(&fixture("plugins.json")).unwrap(),
            SfOutput::Bare(Value::Array(_))
        ));
        assert!(parse(b"not json").is_err());
        assert!(parse("\u{feff}{\"a\":1}".as_bytes()).is_ok());
    }

    #[test]
    fn scrubs_nested_secrets() {
        let mut v =
            json!({"a": [{"AccessToken": "x", "Password": "y", "keep": 1}], "refreshToken": "z"});
        scrub(&mut v);
        assert_eq!(v, json!({"a": [{"keep": 1}]}));
    }
}
