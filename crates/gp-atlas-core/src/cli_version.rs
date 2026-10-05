//! Parsing and comparing `sf` versions (SPEC §4.2: 2.150.6 or newer).

use std::cmp::Ordering;
use std::fmt;

use crate::MIN_CLI_VERSION;

/// A `major.minor.patch[-prerelease]` CLI version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliVersion {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub pre: Option<String>,
}

impl CliVersion {
    /// Parses `2.150.6`, `2.151.0-beta.1` or `@salesforce/cli/2.150.6`.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        let s = s.rsplit('/').next().unwrap_or(s);
        let (core, pre) = match s.split_once('-') {
            Some((c, p)) if !p.is_empty() => (c, Some(p.to_owned())),
            Some(_) => return None,
            None => (s, None),
        };
        let mut parts = core.split('.');
        let n = |p: Option<&str>| p.and_then(|x| x.parse::<u64>().ok());
        let v = Self {
            major: n(parts.next())?,
            minor: n(parts.next())?,
            patch: n(parts.next())?,
            pre,
        };
        parts.next().is_none().then_some(v)
    }

    /// The oldest supported version.
    pub fn minimum() -> Self {
        Self::parse(MIN_CLI_VERSION).expect("valid MIN_CLI_VERSION")
    }

    /// `>= MIN_CLI_VERSION` (a pre-release of the minimum is older than it).
    pub fn is_supported(&self) -> bool {
        *self >= Self::minimum()
    }
}

impl Ord for CliVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| match (&self.pre, &other.pre) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(a), Some(b)) => a.cmp(b),
            })
    }
}

impl PartialOrd for CliVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for CliVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(p) = &self.pre {
            write!(f, "-{p}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> CliVersion {
        CliVersion::parse(s).unwrap()
    }

    #[test]
    fn parses() {
        assert_eq!(v("@salesforce/cli/2.150.6"), v("2.150.6"));
        assert_eq!(v("2.151.0-beta.1").pre.as_deref(), Some("beta.1"));
        for bad in [
            "",
            "2.150",
            "2.150.6.1",
            "x.y.z",
            "2.150.6-",
            "@salesforce/cli/",
        ] {
            assert!(CliVersion::parse(bad).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn minimum_and_newer() {
        assert!(v("2.150.6").is_supported());
        assert!(v("2.150.7").is_supported());
        assert!(v("2.152.14").is_supported());
        assert!(v("2.1000.0").is_supported()); // numeric, not lexical
        assert!(v("3.0.0").is_supported());
        assert!(!v("2.150.5").is_supported());
        assert!(!v("2.99.99").is_supported());
        assert!(!v("1.999.999").is_supported());
        assert!(!v("2.150.6-rc.1").is_supported());
        assert!(v("2.150.7-rc.1").is_supported());
    }
}
