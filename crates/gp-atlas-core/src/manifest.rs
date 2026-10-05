//! The command manifest: a committed, generated subset of `sf commands --json`
//! (SPEC §4.4).
//!
//! `manifest/sf-2.150.6.json` is produced by `tools/extract-manifest` using
//! [`Manifest::extract`]. Never hand-edit it. The same conversion will be used
//! by the Doctor contract check (D4) to compare the live CLI against the
//! committed manifest.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::BASELINE_CLI_VERSION_STRING;

/// Version of the manifest file format.
pub const MANIFEST_SCHEMA: u32 = 1;

/// The committed manifest, embedded at compile time.
pub const EMBEDDED_MANIFEST_JSON: &str = include_str!("../../../manifest/sf-2.150.6.json");

/// Why a command is in the manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// A v1 `ReadOnlyCommand` variant (§4.4 "Runnable").
    Runnable,
    /// Read-only, used by inventory check I3 to display config defaults (§5.2).
    Inventory,
    /// Optional v1.1 read-only command (§4.4 "Optional v1.1 runnable").
    RunnableV1_1,
}

/// Every command id (oclif `:` form) captured in the manifest, with its role.
pub const MANIFEST_COMMANDS: &[(&str, Role)] = &[
    ("version", Role::Runnable),
    ("plugins", Role::Runnable),
    ("commands", Role::Runnable),
    ("org:list", Role::Runnable),
    ("alias:list", Role::Runnable),
    ("package:list", Role::Runnable),
    ("package:version:list", Role::Runnable),
    ("package:version:report", Role::Runnable),
    ("package:version:displayancestry", Role::Runnable),
    ("package:version:displaydependencies", Role::Runnable),
    ("package:version:create:list", Role::Runnable),
    ("package:version:create:report", Role::Runnable),
    ("package:installed:list", Role::Runnable),
    ("package1:version:list", Role::Runnable),
    ("package1:version:display", Role::Runnable),
    ("config:get", Role::Inventory),
    ("package:push-upgrade:list", Role::RunnableV1_1),
    ("package1:version:create:get", Role::RunnableV1_1),
];

/// Errors raised while extracting or loading a manifest.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("`sf commands --json` output is not a JSON array")]
    NotAnArray,
    #[error("command entry without a string `id`")]
    MissingId,
    #[error("command `{command}`: flag `{flag}` has no string `type`")]
    MissingFlagType { command: String, flag: String },
    #[error("commands missing from `sf commands --json`: {0:?}")]
    MissingCommands(Vec<String>),
    #[error("manifest schema {found} is not supported (expected {MANIFEST_SCHEMA})")]
    Schema { found: u32 },
}

/// A generated subset of `sf commands --json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema: u32,
    /// `cliVersion` this manifest describes.
    pub cli_version: String,
    /// How the manifest was produced (informational).
    pub generated_by: String,
    /// Sorted by `id`.
    pub commands: Vec<CommandSpec>,
}

/// One command, as described by `sf commands --json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandSpec {
    /// oclif id, `:`-separated (e.g. `package:version:list`).
    pub id: String,
    /// Plugin that provides the command (`pluginName`).
    pub plugin: Option<String>,
    /// `state` (e.g. `beta`), if any.
    pub state: Option<String>,
    pub hidden: bool,
    /// Alternative command ids (e.g. `force:package:version:list`). Never emitted.
    pub aliases: Vec<String>,
    pub deprecate_aliases: bool,
    /// Sorted by `name`.
    pub flags: Vec<FlagSpec>,
}

/// One flag of a command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlagSpec {
    /// Canonical long name, without dashes.
    pub name: String,
    /// `boolean` or `option`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Short flag character, if any.
    pub char: Option<String>,
    pub required: bool,
    /// Allowed values, if the CLI restricts them.
    pub options: Option<Vec<String>>,
    pub default: Option<Value>,
    pub multiple: bool,
    pub delimiter: Option<String>,
    /// Alternative spellings (e.g. `targetdevhubusername`). Never emitted (§6.1).
    pub aliases: Vec<String>,
    pub deprecate_aliases: bool,
    /// The flag itself is deprecated (e.g. `loglevel`).
    pub deprecated: bool,
    pub hidden: bool,
}

impl Manifest {
    /// Builds the manifest from the parsed output of `sf commands --json`,
    /// keeping exactly the commands in [`MANIFEST_COMMANDS`].
    pub fn extract(commands_json: &Value) -> Result<Self, ManifestError> {
        let all = commands_json.as_array().ok_or(ManifestError::NotAnArray)?;
        let mut commands = Vec::with_capacity(MANIFEST_COMMANDS.len());
        let mut missing = Vec::new();
        for (id, _) in MANIFEST_COMMANDS {
            match all
                .iter()
                .find(|c| c.get("id").and_then(Value::as_str) == Some(id))
            {
                Some(raw) => commands.push(CommandSpec::from_raw(raw)?),
                None => missing.push((*id).to_owned()),
            }
        }
        if !missing.is_empty() {
            return Err(ManifestError::MissingCommands(missing));
        }
        commands.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(Self {
            schema: MANIFEST_SCHEMA,
            cli_version: BASELINE_CLI_VERSION_STRING.to_owned(),
            generated_by: "tools/extract-manifest (from `sf commands --json`)".to_owned(),
            commands,
        })
    }

    /// Parses a manifest file.
    pub fn from_json(json: &str) -> Result<Self, ManifestError> {
        let manifest: Self = serde_json::from_str(json)?;
        if manifest.schema != MANIFEST_SCHEMA {
            return Err(ManifestError::Schema {
                found: manifest.schema,
            });
        }
        Ok(manifest)
    }

    /// The manifest committed in `manifest/sf-2.150.6.json`.
    pub fn embedded() -> Result<Self, ManifestError> {
        Self::from_json(EMBEDDED_MANIFEST_JSON)
    }

    /// Deterministic, pretty-printed JSON with a trailing newline.
    pub fn to_pretty_json(&self) -> String {
        let mut s = serde_json::to_string_pretty(self).expect("manifest serializes");
        s.push('\n');
        s
    }

    /// Looks up a command by id (`package version list` or `package:version:list`).
    pub fn command(&self, id: &str) -> Option<&CommandSpec> {
        let id = crate::blocklist::normalize_id(id);
        self.commands.iter().find(|c| c.id == id)
    }
}

impl CommandSpec {
    /// Converts one entry of `sf commands --json`.
    pub fn from_raw(raw: &Value) -> Result<Self, ManifestError> {
        let id = raw
            .get("id")
            .and_then(Value::as_str)
            .ok_or(ManifestError::MissingId)?
            .to_owned();
        let mut flags = Vec::new();
        if let Some(map) = raw.get("flags").and_then(Value::as_object) {
            for (key, f) in map {
                flags.push(FlagSpec::from_raw(&id, key, f)?);
            }
        }
        flags.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(Self {
            plugin: opt_str(raw, "pluginName"),
            state: opt_str(raw, "state"),
            hidden: bool_of(raw, "hidden"),
            aliases: str_list(raw, "aliases"),
            deprecate_aliases: bool_of(raw, "deprecateAliases"),
            flags,
            id,
        })
    }

    /// Looks up a flag by canonical long name.
    pub fn flag(&self, name: &str) -> Option<&FlagSpec> {
        self.flags.iter().find(|f| f.name == name)
    }
}

impl FlagSpec {
    fn from_raw(command: &str, key: &str, raw: &Value) -> Result<Self, ManifestError> {
        let kind = opt_str(raw, "type").ok_or_else(|| ManifestError::MissingFlagType {
            command: command.to_owned(),
            flag: key.to_owned(),
        })?;
        let options = raw.get("options").and_then(Value::as_array).map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        });
        Ok(Self {
            name: opt_str(raw, "name").unwrap_or_else(|| key.to_owned()),
            kind,
            char: opt_str(raw, "char"),
            required: bool_of(raw, "required"),
            options,
            default: raw.get("default").filter(|v| !v.is_null()).cloned(),
            multiple: bool_of(raw, "multiple"),
            delimiter: opt_str(raw, "delimiter"),
            aliases: str_list(raw, "aliases"),
            deprecate_aliases: bool_of(raw, "deprecateAliases"),
            // `deprecated` is either `true` or an object with a message.
            deprecated: raw
                .get("deprecated")
                .is_some_and(|v| !matches!(v, Value::Null | Value::Bool(false))),
            hidden: bool_of(raw, "hidden"),
        })
    }
}

fn opt_str(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn bool_of(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn str_list(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocklist::is_blocked_command;
    use serde_json::json;

    fn embedded() -> Manifest {
        Manifest::embedded().expect("committed manifest parses")
    }

    #[test]
    fn embedded_manifest_matches_pinned_version() {
        let m = embedded();
        assert_eq!(m.schema, MANIFEST_SCHEMA);
        assert_eq!(m.cli_version, BASELINE_CLI_VERSION_STRING);
    }

    #[test]
    fn embedded_manifest_has_exactly_the_listed_commands() {
        let m = embedded();
        let mut expected: Vec<&str> = MANIFEST_COMMANDS.iter().map(|(id, _)| *id).collect();
        expected.sort_unstable();
        let actual: Vec<&str> = m.commands.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn embedded_manifest_is_canonically_formatted() {
        // Guards against hand edits: the file must round-trip byte-for-byte.
        assert_eq!(embedded().to_pretty_json(), EMBEDDED_MANIFEST_JSON);
    }

    #[test]
    fn no_manifest_command_is_blocklisted() {
        for c in &embedded().commands {
            assert!(!is_blocked_command(&c.id), "{} is blocklisted", c.id);
        }
    }

    #[test]
    fn canonical_flag_names_are_not_deprecated_spellings() {
        // F19: names like `targetdevhubusername` exist only as aliases.
        let deprecated = [
            "targetdevhubusername",
            "targetusername",
            "u",
            "orderby",
            "createdlastdays",
            "modifiedlastdays",
            "packageid",
            "apiversion",
        ];
        for c in &embedded().commands {
            for f in &c.flags {
                assert!(
                    !deprecated.contains(&f.name.as_str()),
                    "{}: canonical flag {} is a deprecated spelling",
                    c.id,
                    f.name
                );
            }
        }
    }

    #[test]
    fn target_flags_are_required_where_spec_says() {
        let m = embedded();
        for id in [
            "package:list",
            "package:version:list",
            "package:version:report",
            "package:version:displayancestry",
            "package:version:displaydependencies",
            "package:version:create:list",
            "package:version:create:report",
        ] {
            let f = m.command(id).and_then(|c| c.flag("target-dev-hub"));
            assert!(f.is_some_and(|f| f.required), "{id} --target-dev-hub");
        }
        for id in [
            "package:installed:list",
            "package1:version:list",
            "package1:version:display",
        ] {
            let f = m.command(id).and_then(|c| c.flag("target-org"));
            assert!(f.is_some_and(|f| f.required), "{id} --target-org");
        }
    }

    #[test]
    fn extract_converts_raw_entries() {
        let raw = json!([
            {"id": "version", "pluginName": "@oclif/plugin-version", "flags": {
                "json": {"name": "json", "type": "boolean", "allowNo": false}
            }},
            {"id": "unrelated", "flags": {}},
        ]);
        let err = Manifest::extract(&raw).unwrap_err();
        assert!(
            matches!(err, ManifestError::MissingCommands(ref m) if m.contains(&"plugins".to_owned()))
        );

        let spec = CommandSpec::from_raw(&json!({
            "id": "package:version:displaydependencies",
            "deprecateAliases": true,
            "flags": {
                "loglevel": {"name": "loglevel", "type": "option", "deprecated": {"message": "x"}, "hidden": true},
                "edge-direction": {"name": "edge-direction", "type": "option",
                    "options": ["root-first", "root-last"], "default": "root-first"},
                "target-dev-hub": {"name": "target-dev-hub", "type": "option", "char": "v",
                    "required": true, "aliases": ["targetdevhubusername"], "deprecateAliases": true}
            }
        }))
        .unwrap();
        let names: Vec<_> = spec.flags.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["edge-direction", "loglevel", "target-dev-hub"]);
        assert!(spec.flag("loglevel").unwrap().deprecated);
        let edge = spec.flag("edge-direction").unwrap();
        assert_eq!(edge.default, Some(json!("root-first")));
        assert_eq!(
            edge.options.as_deref().unwrap(),
            ["root-first", "root-last"]
        );
        let hub = spec.flag("target-dev-hub").unwrap();
        assert!(hub.required && hub.deprecate_aliases);
        assert_eq!(hub.char.as_deref(), Some("v"));
    }

    #[test]
    fn rejects_non_array_input() {
        assert!(matches!(
            Manifest::extract(&json!({"status": 0})),
            Err(ManifestError::NotAnArray)
        ));
    }
}
