//! Replaces identifying values with deterministic fakes for fixture capture.
//!
//! The same real value always maps to the same fake within one capture, so
//! files stay consistent with each other (an alias in `org list` matches the
//! one in a probe's argv). The mapping itself is never written to disk.

use std::collections::{HashMap, HashSet};

use gp_atlas_core::envelope::scrub;
use gp_atlas_core::orgs::Org;
use regex::Regex;
use serde_json::Value;

/// JSON keys whose string values identify a person, org or product.
const IDENTIFYING_KEYS: &[&str] = &[
    "alias",
    "Alias",
    "name",
    "Name",
    "orgName",
    "instanceName",
    "createdOrgInstance",
    "namespacePrefix",
    "NamespacePrefix",
    "namespace",
    "SubscriberPackageNamespace",
    "Description",
    "Package2Name",
    "SubscriberPackageName",
    "SubscriberPackageVersionName",
    "VersionName",
    "Branch",
    "Tag",
    "username",
    "signupUsername",
    "devHubUsername",
    "createdBy",
    "PackageErrorUsername",
    "clientId",
    "ReleaseNotesUrl",
    "PostInstallUrl",
];

/// Keys dropped entirely (local paths, noise).
const DROPPED_KEYS: &[&str] = &["stack"];

/// Hosts that are public Salesforce endpoints, kept as-is.
const PUBLIC_HOSTS: &[&str] = &[
    "login.salesforce.com",
    "test.salesforce.com",
    "developer.salesforce.com",
    "help.salesforce.com",
    "trailhead.salesforce.com",
];

pub struct Sanitizer {
    map: HashMap<String, String>,
    fakes: HashSet<String>,
    counters: HashMap<&'static str, usize>,
    email: Regex,
    host: Regex,
    id: Regex,
    home: Option<String>,
}

impl Default for Sanitizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Sanitizer {
    pub fn new() -> Self {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .ok()
            .filter(|h| h.len() > 3);
        Self {
            map: HashMap::new(),
            fakes: HashSet::new(),
            counters: HashMap::new(),
            email: Regex::new(r"[A-Za-z0-9._%+'-]+@[A-Za-z0-9.-]+\.[A-Za-z0-9.-]+").unwrap(),
            host: Regex::new(r"(?i)\b[a-z0-9][a-z0-9.-]*\.(?:salesforce|force|visualforce|salesforce-setup|cloudforce|database)\.com\b").unwrap(),
            id: Regex::new(r"\b[0-9A-Za-z]{15}(?:[0-9A-Za-z]{3})?\b").unwrap(),
            home,
        }
    }

    fn next(&mut self, kind: &'static str) -> usize {
        let c = self.counters.entry(kind).or_insert(0);
        *c += 1;
        *c
    }

    fn remember(&mut self, real: &str, fake: String) -> String {
        self.fakes.insert(fake.clone());
        self.map.insert(real.to_owned(), fake.clone());
        fake
    }

    fn fake_for(&mut self, real: &str, kind: &'static str) -> String {
        if let Some(f) = self.map.get(real) {
            return f.clone();
        }
        if self.fakes.contains(real) {
            return real.to_owned();
        }
        let n = self.next(kind);
        let fake = match kind {
            "email" => format!("user{n:04}@example.com"),
            "host" => format!("org{n:04}.my.salesforce.com"),
            "id" => {
                let prefix: String = real.chars().take(3).collect();
                let body = format!("{n:0width$}", width = real.len() - 3);
                format!("{prefix}{body}")
            }
            _ => format!("fake{n:04}"),
        };
        self.remember(real, fake)
    }

    /// Seeds the mapping from the org inventory so free text (error messages,
    /// argv) is sanitized consistently.
    pub fn seed_orgs(&mut self, orgs: &[Org]) {
        for o in orgs {
            if o.username.contains('@') {
                self.fake_for(&o.username, "email");
            } else {
                self.fake_for(&o.username, "word");
            }
            if let Some(a) = &o.alias {
                self.fake_for(a, "word");
            }
            if let Some(id) = &o.org_id {
                self.fake_for(id, "id");
            }
        }
    }

    fn is_id(token: &str) -> bool {
        let digits = token.chars().filter(char::is_ascii_digit).count();
        let letters = token.chars().filter(char::is_ascii_alphabetic).count();
        digits >= 3 && letters >= 1
    }

    /// Sanitizes free text.
    pub fn text(&mut self, s: &str) -> String {
        let mut out = s.to_owned();
        if let Some(home) = &self.home {
            out = out.replace(home.as_str(), "~");
        }
        // Known values (longest first so "a.b" wins over "a").
        let mut known: Vec<(String, String)> = self
            .map
            .iter()
            .filter(|(k, _)| k.len() >= 4)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        known.sort_by_key(|(k, _)| std::cmp::Reverse(k.len()));
        for (real, fake) in known {
            if out.contains(&real) {
                out = out.replace(&real, &fake);
            }
        }
        let emails: Vec<String> = self
            .email
            .find_iter(&out)
            // A sentence-final "." is not part of the address.
            .map(|m| m.as_str().trim_end_matches('.').to_owned())
            .collect();
        for e in emails {
            if !e.ends_with("@example.com") {
                let f = self.fake_for(&e, "email");
                out = out.replace(&e, &f);
            }
        }
        let hosts: Vec<String> = self
            .host
            .find_iter(&out)
            .map(|m| m.as_str().to_owned())
            .collect();
        for h in hosts {
            let lower = h.to_ascii_lowercase();
            if PUBLIC_HOSTS.contains(&lower.as_str()) || self.fakes.contains(&h) {
                continue;
            }
            let f = self.fake_for(&h, "host");
            out = out.replace(&h, &f);
        }
        let ids: Vec<String> = self
            .id
            .find_iter(&out)
            .map(|m| m.as_str().to_owned())
            .filter(|t| Self::is_id(t))
            .collect();
        for id in ids {
            if self.fakes.contains(&id) {
                continue;
            }
            let f = self.fake_for(&id, "id");
            out = out.replace(&id, &f);
        }
        out
    }

    /// Sanitizes a JSON value in place (also scrubs secrets).
    pub fn value(&mut self, v: &mut Value) {
        scrub(v);
        self.walk(v);
    }

    fn walk(&mut self, v: &mut Value) {
        match v {
            Value::Object(o) => {
                o.retain(|k, _| !DROPPED_KEYS.contains(&k.as_str()));
                for (k, val) in o.iter_mut() {
                    match val {
                        Value::String(s)
                            if !s.is_empty() && IDENTIFYING_KEYS.contains(&k.as_str()) =>
                        {
                            *s = if s.contains('@') {
                                self.fake_for(s, "email")
                            } else if k == "Alias" && s.contains(',') {
                                s.split(',')
                                    .map(|a| self.fake_for(a, "word"))
                                    .collect::<Vec<_>>()
                                    .join(",")
                            } else {
                                self.fake_for(s, "word")
                            };
                        }
                        _ => self.walk(val),
                    }
                }
            }
            Value::Array(a) => a.iter_mut().for_each(|x| self.walk(x)),
            Value::String(s) => *s = self.text(s),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn consistent_and_secret_free() {
        let mut s = Sanitizer::new();
        let mut v = json!({
            "username": "jane.doe@acme.com",
            "alias": "acme-hub",
            "orgId": "00D5g000004AbCdEAK",
            "instanceUrl": "https://acme.my.salesforce.com",
            "loginUrl": "https://login.salesforce.com",
            "accessToken": "00D!secret",
            "stack": "/Users/jane/x.js",
            "message": "No authorization information found for jane.doe@acme.com.",
            "Version": "1.2.0.3",
            "MajorVersion": 1
        });
        s.value(&mut v);
        let out = v.to_string();
        assert!(!out.contains("acme"), "{out}");
        assert!(!out.contains("jane"), "{out}");
        assert!(!out.contains("accessToken"));
        assert!(!out.contains("stack"));
        assert!(out.contains("https://login.salesforce.com"));
        assert_eq!(v["Version"], "1.2.0.3");
        assert_eq!(v["MajorVersion"], 1);
        // Same email → same fake in a key and in free text.
        let fake = v["username"].as_str().unwrap().to_owned();
        assert!(v["message"].as_str().unwrap().contains(&fake));
        // IDs keep their prefix and length.
        let id = v["orgId"].as_str().unwrap();
        assert!(id.starts_with("00D") && id.len() == 18, "{id}");
    }

    #[test]
    fn idempotent_on_fakes() {
        let mut s = Sanitizer::new();
        let a = s.text("id 04t5g000000AbCdAAA here");
        let b = s.text(&a);
        assert_eq!(a, b);
    }

    #[test]
    fn plain_words_are_not_ids() {
        let mut s = Sanitizer::new();
        assert_eq!(s.text("authentication failure"), "authentication failure");
    }
}
