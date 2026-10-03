//! Validated values that may reach argv (SPEC §9.2: reject, don't sanitize).

use std::fmt;

/// A value failed validation and must not reach argv.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid {what}: {value:?} ({rule})")]
pub struct ValidationError {
    pub what: &'static str,
    pub value: String,
    pub rule: &'static str,
}

impl ValidationError {
    fn new(what: &'static str, value: &str, rule: &'static str) -> Self {
        Self {
            what,
            value: value.to_owned(),
            rule,
        }
    }
}

fn all_chars(s: &str, ok: impl Fn(char) -> bool) -> bool {
    s.chars().all(ok)
}

/// Org alias or username: `^[A-Za-z0-9._@+\-]{1,120}$`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OrgRef(String);

impl OrgRef {
    pub fn new(s: &str) -> Result<Self, ValidationError> {
        let ok = (1..=120).contains(&s.len())
            && all_chars(s, |c| c.is_ascii_alphanumeric() || "._@+-".contains(c));
        if ok {
            Ok(Self(s.to_owned()))
        } else {
            Err(ValidationError::new(
                "org alias/username",
                s,
                "1-120 chars of A-Z a-z 0-9 . _ @ + -",
            ))
        }
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for OrgRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A Salesforce record id with a fixed key prefix.
macro_rules! id_type {
    ($name:ident, $prefix:literal, $doc:literal, both) => {
        id_type!(@def $name, $prefix, $doc, &[15, 18], "prefix + 15 or 18 alphanumeric chars");
    };
    ($name:ident, $prefix:literal, $doc:literal, only18) => {
        id_type!(@def $name, $prefix, $doc, &[18], "prefix + exactly 18 alphanumeric chars");
    };
    (@def $name:ident, $prefix:literal, $doc:literal, $lens:expr, $rule:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name(String);

        impl $name {
            pub const PREFIX: &'static str = $prefix;

            pub fn new(s: &str) -> Result<Self, ValidationError> {
                let lens: &[usize] = $lens;
                if lens.contains(&s.len())
                    && s.starts_with($prefix)
                    && all_chars(s, |c| c.is_ascii_alphanumeric())
                {
                    Ok(Self(s.to_owned()))
                } else {
                    Err(ValidationError::new(concat!($prefix, " id"), s, $rule))
                }
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

id_type!(Id0Ho, "0Ho", "2GP package id.", both);
id_type!(Id04t, "04t", "Subscriber package version id.", both);
id_type!(Id08c, "08c", "Package version create request id.", both);
id_type!(Id0HD, "0HD", "1GP upload request id.", both);
// F24: `package1 version list --package-id` only accepts 18 characters.
id_type!(
    Id033,
    "033",
    "1GP metadata package id (18 chars only, F24).",
    only18
);

/// A project package alias (`packageAliases` key): `^[A-Za-z0-9 _.@+\-:/]{1,200}$`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PackageAlias(String);

impl PackageAlias {
    pub fn new(s: &str) -> Result<Self, ValidationError> {
        let ok = (1..=200).contains(&s.len())
            && all_chars(s, |c| c.is_ascii_alphanumeric() || " _.@+-:/".contains(c));
        if ok {
            Ok(Self(s.to_owned()))
        } else {
            Err(ValidationError::new(
                "package alias",
                s,
                "1-200 chars of A-Z a-z 0-9 space _ . @ + - : /",
            ))
        }
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Branch name: `^[A-Za-z0-9._\-/]{1,100}$` (F14: pasted into SOQL quotes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Branch(String);

impl Branch {
    pub fn new(s: &str) -> Result<Self, ValidationError> {
        let ok = (1..=100).contains(&s.len())
            && all_chars(s, |c| c.is_ascii_alphanumeric() || "._-/".contains(c));
        if ok {
            Ok(Self(s.to_owned()))
        } else {
            Err(ValidationError::new(
                "branch",
                s,
                "1-100 chars of A-Z a-z 0-9 . _ - / (no quotes or spaces)",
            ))
        }
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// `--api-version`: `^\d{2}\.\d$`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiVersion(String);

impl ApiVersion {
    pub fn new(s: &str) -> Result<Self, ValidationError> {
        let b = s.as_bytes();
        let ok = b.len() == 4
            && b[0].is_ascii_digit()
            && b[1].is_ascii_digit()
            && b[2] == b'.'
            && b[3].is_ascii_digit();
        if ok {
            Ok(Self(s.to_owned()))
        } else {
            Err(ValidationError::new("api version", s, "NN.N, e.g. 62.0"))
        }
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A `--package` value that may be an id or a project alias (§6.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageRef<I> {
    Id(I),
    Alias(PackageAlias),
}

impl<I: fmt::Display> fmt::Display for PackageRef<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Id(i) => i.fmt(f),
            Self::Alias(a) => f.write_str(a.as_str()),
        }
    }
}

/// `--package` for `displayancestry`: 0Ho, 04t or alias.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AncestryTarget {
    Package(Id0Ho),
    Version(Id04t),
    Alias(PackageAlias),
}

impl fmt::Display for AncestryTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Package(i) => i.fmt(f),
            Self::Version(i) => i.fmt(f),
            Self::Alias(a) => f.write_str(a.as_str()),
        }
    }
}

/// `--package` for `displaydependencies`: 04t, 08c or alias.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyTarget {
    Version(Id04t),
    Request(Id08c),
    Alias(PackageAlias),
}

impl fmt::Display for DependencyTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Version(i) => i.fmt(f),
            Self::Request(i) => i.fmt(f),
            Self::Alias(a) => f.write_str(a.as_str()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn org_refs() {
        assert!(OrgRef::new("my-hub").is_ok());
        assert!(OrgRef::new("user.name+dev@example.com").is_ok());
        for bad in ["", "a b", "x;rm", "a\"b", "$HOME", "a'b", &"x".repeat(121)] {
            assert!(OrgRef::new(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn ids() {
        assert!(Id0Ho::new("0Ho000000000001").is_ok());
        assert!(Id0Ho::new("0Ho000000000001AAA").is_ok());
        assert!(Id0Ho::new("0Ho00000000001").is_err()); // 14
        assert!(Id0Ho::new("04t000000000001AAA").is_err()); // prefix
        assert!(Id04t::new("04t00000000000'AAA").is_err());
        // F24: 033 must be 18.
        assert!(Id033::new("033000000000001").is_err());
        assert!(Id033::new("033000000000001AAA").is_ok());
    }

    #[test]
    fn branch_and_api_version() {
        assert!(Branch::new("feature/x-1.2").is_ok());
        assert!(Branch::new("a' OR 'x'='x").is_err());
        assert!(Branch::new("has space").is_err());
        assert!(ApiVersion::new("62.0").is_ok());
        assert!(ApiVersion::new("62").is_err());
        assert!(ApiVersion::new("6.20").is_err());
    }

    #[test]
    fn package_alias_allows_spaces_but_not_quotes() {
        assert!(PackageAlias::new("My Package@1.2.0-3").is_ok());
        assert!(PackageAlias::new("bad\"alias").is_err());
        assert!(PackageAlias::new("a,b").is_err());
    }
}
