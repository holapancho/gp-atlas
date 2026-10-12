//! Project package aliases from `sfdx-project.json` (SPEC §6.2, §6.3).
//!
//! The CLI resolves `packageAliases` only when its cwd is inside the project
//! (F13) and `package list` never fills its `Alias` column (F12), so GP Atlas
//! reads them itself. The file is only ever **read** (AC-12); only
//! `packageAliases` is used.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::ids::{Id0Ho, Id04t, PackageAlias};

pub const PROJECT_FILE: &str = "sfdx-project.json";

/// Larger files are refused (a real project file is a few KB).
const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("no {PROJECT_FILE} in {0} or any parent folder")]
    NotFound(PathBuf),
    #[error("cannot read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{0} is larger than 5 MB; not a project file")]
    TooLarge(PathBuf),
    #[error("{path} is not valid JSON: {message}")]
    Json { path: PathBuf, message: String },
}

/// What an alias points to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AliasKind {
    /// `0Ho…`: a package.
    Package,
    /// `04t…`: a package version.
    Version,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasEntry {
    pub alias: String,
    /// The ID as written in the file (15 or 18 chars).
    pub id: String,
    pub kind: AliasKind,
}

/// An entry GP Atlas could not use, with the reason. Both strings are made
/// printable (control characters replaced) because they come from a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    pub alias: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct Project {
    /// Full path of `sfdx-project.json`.
    pub file: PathBuf,
    /// Entries in file order (the first alias of an ID is its "primary").
    pub entries: Vec<AliasEntry>,
    pub skipped: Vec<Skipped>,
    by_id15: HashMap<String, Vec<usize>>,
}

/// The folder that holds `sfdx-project.json`: `start` or its closest parent
/// (the CLI searches parents up to the filesystem root, F32).
pub fn find_project_dir(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|d| d.join(PROJECT_FILE).is_file())
        .map(Path::to_path_buf)
}

/// First 15 characters: joins 15- and 18-char IDs (case-sensitive, §6.2).
pub fn id15(id: &str) -> &str {
    id.get(..15).unwrap_or(id)
}

/// Replaces control characters so file content cannot drive the terminal.
pub fn printable(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { '�' } else { c })
        .collect()
}

impl Project {
    /// Finds `sfdx-project.json` from `start` (a folder or the file itself)
    /// upwards and reads its `packageAliases`.
    pub fn discover(start: &Path) -> Result<Self, ProjectError> {
        if start.is_file() {
            return Self::load(start);
        }
        let dir =
            find_project_dir(start).ok_or_else(|| ProjectError::NotFound(start.to_path_buf()))?;
        Self::load(&dir.join(PROJECT_FILE))
    }

    /// Reads one `sfdx-project.json`. Read-only.
    pub fn load(file: &Path) -> Result<Self, ProjectError> {
        let read_err = |source| ProjectError::Read {
            path: file.to_path_buf(),
            source,
        };
        let len = std::fs::metadata(file).map_err(read_err)?.len();
        if len > MAX_FILE_BYTES {
            return Err(ProjectError::TooLarge(file.to_path_buf()));
        }
        let bytes = std::fs::read(file).map_err(read_err)?;
        Self::parse(file, &bytes)
    }

    /// Parses file content (exposed for tests).
    pub fn parse(file: &Path, bytes: &[u8]) -> Result<Self, ProjectError> {
        let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
        let json: ProjectJson = serde_json::from_slice(bytes).map_err(|e| ProjectError::Json {
            path: file.to_path_buf(),
            message: e.to_string(),
        })?;
        let mut p = Project {
            file: file.to_path_buf(),
            entries: Vec::new(),
            skipped: Vec::new(),
            by_id15: HashMap::new(),
        };
        for (alias, value) in &json.aliases {
            let skip = |reason: &str| Skipped {
                alias: printable(alias),
                reason: reason.to_owned(),
            };
            if PackageAlias::new(alias).is_err() {
                p.skipped.push(skip(
                    "alias has characters outside A-Z a-z 0-9 space _ . @ + - : /",
                ));
                continue;
            }
            let Some(id) = value.as_str() else {
                p.skipped.push(skip("value is not a string"));
                continue;
            };
            let kind = if Id0Ho::new(id).is_ok() {
                AliasKind::Package
            } else if Id04t::new(id).is_ok() {
                AliasKind::Version
            } else {
                p.skipped.push(Skipped {
                    alias: printable(alias),
                    reason: format!("value {:?} is not a 0Ho or 04t id", printable(id)),
                });
                continue;
            };
            p.by_id15
                .entry(id15(id).to_owned())
                .or_default()
                .push(p.entries.len());
            p.entries.push(AliasEntry {
                alias: alias.clone(),
                id: id.to_owned(),
                kind,
            });
        }
        Ok(p)
    }

    /// The folder that holds the file.
    pub fn dir(&self) -> &Path {
        self.file.parent().unwrap_or(Path::new("."))
    }

    /// All aliases of an ID, primary (first in the file) first.
    pub fn aliases_for(&self, id: &str) -> Vec<&str> {
        self.by_id15
            .get(id15(id))
            .map(|ix| ix.iter().map(|&i| self.entries[i].alias.as_str()).collect())
            .unwrap_or_default()
    }

    pub fn count(&self, kind: AliasKind) -> usize {
        self.entries.iter().filter(|e| e.kind == kind).count()
    }

    /// IDs with more than one alias, in file order.
    pub fn duplicates(&self) -> Vec<(&str, Vec<&str>)> {
        let mut seen = BTreeSet::new();
        self.entries
            .iter()
            .filter(|e| seen.insert(id15(&e.id)))
            .filter_map(|e| {
                let a = self.aliases_for(&e.id);
                (a.len() > 1).then_some((e.id.as_str(), a))
            })
            .collect()
    }

    /// Aliases of `kind` whose ID is not among `known` (IDs from the current
    /// Dev Hub). Possibly another Dev Hub, or a deleted package/version.
    pub fn orphans<'a>(
        &self,
        kind: AliasKind,
        known: impl IntoIterator<Item = &'a str>,
    ) -> Vec<&AliasEntry> {
        let known: BTreeSet<&str> = known.into_iter().map(id15).collect();
        self.entries
            .iter()
            .filter(|e| e.kind == kind && !known.contains(id15(&e.id)))
            .collect()
    }
}

/// The top level of `sfdx-project.json`: only `packageAliases`, in file order
/// (`serde_json::Map` would sort the keys and lose which alias is primary).
struct ProjectJson {
    aliases: Vec<(String, Value)>,
}

impl<'de> serde::Deserialize<'de> for ProjectJson {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Top;
        struct Aliases;
        impl<'de> serde::de::Visitor<'de> for Top {
            type Value = ProjectJson;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a JSON object")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut m: A,
            ) -> Result<ProjectJson, A::Error> {
                let mut aliases = Vec::new();
                while let Some(key) = m.next_key::<String>()? {
                    if key == "packageAliases" {
                        aliases = m.next_value_seed(Aliases)?;
                    } else {
                        m.next_value::<serde::de::IgnoredAny>()?;
                    }
                }
                Ok(ProjectJson { aliases })
            }
        }
        impl<'de> serde::de::DeserializeSeed<'de> for Aliases {
            type Value = Vec<(String, Value)>;
            fn deserialize<D: serde::Deserializer<'de>>(
                self,
                d: D,
            ) -> Result<Self::Value, D::Error> {
                d.deserialize_any(self)
            }
        }
        impl<'de> serde::de::Visitor<'de> for Aliases {
            type Value = Vec<(String, Value)>;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("packageAliases")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut m: A,
            ) -> Result<Self::Value, A::Error> {
                let mut v = Vec::new();
                while let Some(e) = m.next_entry::<String, Value>()? {
                    v.push(e);
                }
                Ok(v)
            }
            // Anything other than an object (e.g. `[]`) means no aliases.
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut s: A,
            ) -> Result<Self::Value, A::Error> {
                while s.next_element::<serde::de::IgnoredAny>()?.is_some() {}
                Ok(Vec::new())
            }
            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(Vec::new())
            }
            fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E> {
                Ok(Vec::new())
            }
            fn visit_str<E>(self, _: &str) -> Result<Self::Value, E> {
                Ok(Vec::new())
            }
            fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E> {
                Ok(Vec::new())
            }
            fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E> {
                Ok(Vec::new())
            }
            fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E> {
                Ok(Vec::new())
            }
        }
        d.deserialize_map(Top)
    }
}

/// The alias the CLI writes for a new version (`package version create` /
/// `promote`): `<first package alias, else package name>@M.m.p-build[-branch]`.
pub fn version_alias(package: &str, version: &str, branch: Option<&str>) -> String {
    let mut parts = version.split('.');
    let (Some(ma), Some(mi), Some(pa)) = (parts.next(), parts.next(), parts.next()) else {
        return format!("{package}@{version}");
    };
    let mut s = format!("{package}@{ma}.{mi}.{pa}");
    if let Some(b) = parts.next().filter(|b| !b.is_empty() && *b != "0") {
        s.push('-');
        s.push_str(b);
    }
    if let Some(br) = branch.filter(|b| !b.is_empty()) {
        s.push('-');
        s.push_str(br);
    }
    s
}

/// `packageAliases` lines for copying (`"alias": "id",`), JSON-escaped.
pub fn snippet(entries: &[(String, String)]) -> String {
    entries
        .iter()
        .map(|(a, id)| {
            format!(
                "{}: {},",
                Value::String(a.clone()),
                Value::String(id.clone())
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    const PKG: &str = "0Ho000000000001CAA";
    const PKG2: &str = "0Ho000000000002CAA";
    const V1: &str = "04t000000000001AAA";
    const V2: &str = "04t000000000002AAA";

    fn project(json: &str) -> Project {
        Project::parse(Path::new("/p/sfdx-project.json"), json.as_bytes()).unwrap()
    }

    fn sample() -> Project {
        project(&format!(
            r#"{{
              "packageDirectories": [{{"path": "force-app", "package": "MyApp"}}],
              "packageAliases": {{
                "MyApp": "{PKG}",
                "Other": "{PKG2}",
                "MyApp@1.0.0-1": "{V1}",
                "MyApp@1.0.0-1-old": "{}",
                "Bad\u001b[31m": "{V2}",
                "NotAnId": "a0x000000000001",
                "Number": 3
              }}
            }}"#,
            &V1[..15]
        ))
    }

    #[test]
    fn reads_aliases_in_file_order_and_skips_bad_entries() {
        let p = sample();
        let names: Vec<_> = p.entries.iter().map(|e| e.alias.as_str()).collect();
        assert_eq!(
            names,
            ["MyApp", "Other", "MyApp@1.0.0-1", "MyApp@1.0.0-1-old"]
        );
        assert_eq!(p.count(AliasKind::Package), 2);
        assert_eq!(p.count(AliasKind::Version), 2);
        assert_eq!(p.skipped.len(), 3);
        // Control characters never reach the screen.
        assert!(p.skipped.iter().all(|s| !s.alias.contains('\u{1b}')));
        assert!(p.skipped[1].reason.contains("not a 0Ho or 04t"));
    }

    #[test]
    fn joins_15_and_18_char_ids() {
        let p = sample();
        assert_eq!(p.aliases_for(V1), ["MyApp@1.0.0-1", "MyApp@1.0.0-1-old"]);
        assert_eq!(p.aliases_for(&V1[..15]), p.aliases_for(V1));
        assert_eq!(p.aliases_for(PKG), ["MyApp"]);
        assert!(p.aliases_for(V2).is_empty());
        // Case-sensitive: a different case is a different id.
        assert!(p.aliases_for(&PKG.to_lowercase()).is_empty());
    }

    #[test]
    fn duplicates_and_orphans() {
        let p = sample();
        let d = p.duplicates();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].1, ["MyApp@1.0.0-1", "MyApp@1.0.0-1-old"]);
        let o = p.orphans(AliasKind::Package, [PKG]);
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].alias, "Other");
        assert!(p.orphans(AliasKind::Version, [V1]).is_empty());
    }

    #[test]
    fn missing_or_empty_aliases_and_bom() {
        assert!(project("{}").entries.is_empty());
        assert!(project(r#"{"packageAliases": []}"#).entries.is_empty());
        let bom = format!("\u{feff}{{\"packageAliases\": {{\"A\": \"{PKG}\"}}}}");
        let p = Project::parse(Path::new("x"), bom.as_bytes()).unwrap();
        assert_eq!(p.entries.len(), 1);
        assert!(matches!(
            Project::parse(Path::new("x"), b"{ nope"),
            Err(ProjectError::Json { .. })
        ));
    }

    #[test]
    fn version_alias_follows_the_cli() {
        assert_eq!(version_alias("MyApp", "1.2.0.3", None), "MyApp@1.2.0-3");
        assert_eq!(
            version_alias("MyApp", "1.2.0.3", Some("dev")),
            "MyApp@1.2.0-3-dev"
        );
        assert_eq!(
            version_alias("My App", "2.0.1.1", Some("")),
            "My App@2.0.1-1"
        );
        assert_eq!(version_alias("X", "1.2", None), "X@1.2");
    }

    #[test]
    fn snippet_is_json_escaped() {
        let s = snippet(&[
            ("MyApp@1.2.0-3".into(), V1.into()),
            ("Say \"hi\"".into(), V2.into()),
        ]);
        assert_eq!(
            s,
            format!("\"MyApp@1.2.0-3\": \"{V1}\",\n\"Say \\\"hi\\\"\": \"{V2}\",")
        );
    }

    /// AC-12: GP Atlas never writes to `sfdx-project.json`.
    #[test]
    fn discover_walks_up_and_never_writes() {
        let root = std::env::temp_dir().join(format!("gp-atlas-project-{}", std::process::id()));
        let nested = root.join("force-app/main/default");
        std::fs::create_dir_all(&nested).unwrap();
        let file = root.join(PROJECT_FILE);
        let content = format!("{{\"packageAliases\": {{\"MyApp\": \"{PKG}\"}}}}\n");
        std::fs::write(&file, &content).unwrap();
        let before = std::fs::metadata(&file).unwrap().modified().unwrap();

        let p = Project::discover(&nested).unwrap();
        assert_eq!(p.file, file);
        assert_eq!(p.dir(), root);
        assert_eq!(p.aliases_for(PKG), ["MyApp"]);
        assert_eq!(Project::discover(&file).unwrap().entries, p.entries);

        assert_eq!(std::fs::read_to_string(&file).unwrap(), content);
        assert_eq!(
            std::fs::metadata(&file).unwrap().modified().unwrap(),
            before
        );
        std::fs::remove_dir_all(&root).unwrap();

        assert!(matches!(
            Project::discover(&root),
            Err(ProjectError::NotFound(_))
        ));
    }
}
