//! Ancestry and dependency graphs from the CLI's DOT output
//! (docs/ideas/ANCESTRY_AND_DEPENDENCIES.md).
//!
//! The CLI prints exactly two DOT shapes (plugin-packaging 3.0.6 /
//! @salesforce/packaging 5.0.7 source):
//!
//! * `displayancestry --dot-code`: `strict graph G {` with nodes
//!   `node<04t> [label="M.m.p.b"]` and parent→child edges `node<a> -- node<b>`.
//!   For a package (0Ho) it contains **every root** (the JSON form keeps only
//!   the first root), released versions only.
//! * `displaydependencies --edge-direction root-last`: `strict digraph G {`
//!   with nodes `node_<04t> [label="Name@M.m.p.b" color="green"]` and edges
//!   `node_<a> -> node_<b>` meaning *a is installed before b*.
//!
//! The parser accepts only that subset; anything else is reported, not guessed.

use std::collections::{BTreeMap, HashMap, HashSet};

/// One node: `id` is a 04t (or `VERSION_BEING_BUILT`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub id: String,
    pub label: String,
    /// Marked by the CLI (`color="green"`): the selected version and its
    /// direct dependencies.
    pub highlighted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Graph {
    pub directed: bool,
    pub nodes: Vec<Node>,
    /// `(from, to)`: ancestry = parent→child; dependencies = install-before.
    pub edges: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GraphError {
    #[error("not DOT output from sf: {0}")]
    Format(String),
    #[error("the dependency graph has a cycle")]
    Cycle,
}

fn node_id(token: &str) -> Option<&str> {
    let t = token.trim();
    let rest = t.strip_prefix("node")?;
    let rest = rest.strip_prefix('_').unwrap_or(rest);
    (!rest.is_empty() && rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        .then_some(rest)
}

fn attr<'a>(attrs: &'a str, key: &str) -> Option<&'a str> {
    let start = attrs.find(&format!("{key}=\""))? + key.len() + 2;
    let end = attrs[start..].find('"')? + start;
    Some(&attrs[start..end])
}

/// Parses the CLI's DOT output (see module docs).
pub fn parse_dot(dot: &str) -> Result<Graph, GraphError> {
    let text = dot.trim();
    let directed = if text.starts_with("strict digraph G {") {
        true
    } else if text.starts_with("strict graph G {") {
        false
    } else {
        return Err(GraphError::Format(text.chars().take(40).collect()));
    };
    if !text.ends_with('}') {
        return Err(GraphError::Format("missing closing brace".into()));
    }
    let body = &text[text.find('{').unwrap_or(0) + 1..text.len() - 1];
    let arrow = if directed { "->" } else { "--" };
    let mut nodes: Vec<Node> = Vec::new();
    let mut edges = Vec::new();
    for raw in body.lines() {
        let line = raw.trim().trim_end_matches(';');
        if line.is_empty() {
            continue;
        }
        if let Some((a, b)) = line.split_once(arrow) {
            match (node_id(a), node_id(b)) {
                (Some(a), Some(b)) => edges.push((a.to_owned(), b.to_owned())),
                _ => return Err(GraphError::Format(line.to_owned())),
            }
        } else if let Some((head, rest)) = line.split_once('[') {
            let id = node_id(head).ok_or_else(|| GraphError::Format(line.to_owned()))?;
            let label = attr(rest, "label").unwrap_or(id).to_owned();
            nodes.push(Node {
                id: id.to_owned(),
                label,
                highlighted: attr(rest, "color").is_some(),
            });
        } else {
            return Err(GraphError::Format(line.to_owned()));
        }
    }
    // Edges may mention nodes without a node line; add them with their id as label.
    for (a, b) in &edges {
        for id in [a, b] {
            if !nodes.iter().any(|n| &n.id == id) {
                nodes.push(Node {
                    id: id.clone(),
                    label: id.clone(),
                    highlighted: false,
                });
            }
        }
    }
    Ok(Graph {
        directed,
        nodes,
        edges,
    })
}

/// One line of a rendered tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeLine {
    /// Box-drawing prefix, e.g. `"│  ├─ "`.
    pub prefix: String,
    pub depth: usize,
    pub id: String,
}

impl Graph {
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    fn children_map(&self, reverse: bool) -> HashMap<&str, Vec<&str>> {
        let mut m: HashMap<&str, Vec<&str>> = HashMap::new();
        for (a, b) in &self.edges {
            let (p, c) = if reverse { (b, a) } else { (a, b) };
            m.entry(p.as_str()).or_default().push(c.as_str());
        }
        m
    }

    /// Ids with no incoming edge (in the given orientation), in node order.
    fn roots(&self, reverse: bool) -> Vec<&str> {
        let has_parent: HashSet<&str> = self
            .edges
            .iter()
            .map(|(a, b)| if reverse { a.as_str() } else { b.as_str() })
            .collect();
        self.nodes
            .iter()
            .map(|n| n.id.as_str())
            .filter(|id| !has_parent.contains(id))
            .collect()
    }

    /// Renders the graph as a forest. `reverse` flips edge direction (used to
    /// show dependencies from the selected package down).
    pub fn forest(&self, reverse: bool) -> Vec<TreeLine> {
        let children = self.children_map(reverse);
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        let roots = self.roots(reverse);
        let n = roots.len();
        for (i, r) in roots.iter().enumerate() {
            walk(r, "", i + 1 == n, 0, &children, &mut seen, &mut out, true);
        }
        out
    }

    /// Path from `id` up to its root (ancestry: `id`, parent, grandparent…).
    pub fn path_to_root(&self, id: &str) -> Vec<String> {
        let parent: HashMap<&str, &str> = self
            .edges
            .iter()
            .map(|(a, b)| (b.as_str(), a.as_str()))
            .collect();
        let mut path = Vec::new();
        let mut cur = id;
        if self.node(id).is_none() {
            return path;
        }
        while path.len() <= self.nodes.len() {
            path.push(cur.to_owned());
            match parent.get(cur) {
                Some(p) => cur = p,
                None => break,
            }
        }
        path
    }

    /// Number of direct children of `id` (ancestry: versions built on it).
    pub fn child_count(&self, id: &str) -> usize {
        self.edges.iter().filter(|(a, _)| a == id).count()
    }

    /// Install order for a dependency graph (`a -> b`: install a before b).
    /// Ties keep the CLI's node order.
    pub fn install_order(&self) -> Result<Vec<&Node>, GraphError> {
        let mut indeg: BTreeMap<usize, usize> = BTreeMap::new();
        let index: HashMap<&str, usize> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.as_str(), i))
            .collect();
        for i in 0..self.nodes.len() {
            indeg.insert(i, 0);
        }
        for (_, b) in &self.edges {
            if let Some(i) = index.get(b.as_str()) {
                *indeg.get_mut(i).expect("index") += 1;
            }
        }
        let children = self.children_map(false);
        let mut out = Vec::new();
        let mut ready: Vec<usize> = indeg
            .iter()
            .filter(|(_, d)| **d == 0)
            .map(|(i, _)| *i)
            .collect();
        while let Some(pos) = ready
            .iter()
            .enumerate()
            .min_by_key(|(_, i)| **i)
            .map(|(p, _)| p)
        {
            let i = ready.remove(pos);
            out.push(&self.nodes[i]);
            for c in children
                .get(self.nodes[i].id.as_str())
                .into_iter()
                .flatten()
            {
                if let Some(ci) = index.get(c) {
                    let d = indeg.get_mut(ci).expect("index");
                    *d -= 1;
                    if *d == 0 {
                        ready.push(*ci);
                    }
                }
            }
        }
        if out.len() == self.nodes.len() {
            Ok(out)
        } else {
            Err(GraphError::Cycle)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn walk<'a>(
    id: &'a str,
    indent: &str,
    last: bool,
    depth: usize,
    children: &HashMap<&'a str, Vec<&'a str>>,
    seen: &mut HashSet<&'a str>,
    out: &mut Vec<TreeLine>,
    top: bool,
) {
    let branch = if top {
        ""
    } else if last {
        "└─ "
    } else {
        "├─ "
    };
    out.push(TreeLine {
        prefix: format!("{indent}{branch}"),
        depth,
        id: id.to_owned(),
    });
    if !seen.insert(id) {
        return; // shared sub-graph already shown above
    }
    let next = if top {
        String::new()
    } else {
        format!("{indent}{}", if last { "   " } else { "│  " })
    };
    let kids = children.get(id).cloned().unwrap_or_default();
    let n = kids.len();
    for (i, k) in kids.iter().enumerate() {
        walk(k, &next, i + 1 == n, depth + 1, children, seen, out, false);
    }
}

/// `Name@1.2.0.3` → (`Name`, `1.2.0.3`). Version may be `VERSION_BEING_BUILT`.
pub fn split_label(label: &str) -> (&str, &str) {
    let label = label.split(" (").next().unwrap_or(label); // drop verbose " (04t…)"
    label.rsplit_once('@').unwrap_or((label, ""))
}

/// Compares dotted versions numerically (`1.10.0.1` > `1.2.0.9`).
pub fn cmp_versions(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    let p = |s: &str| -> Option<Vec<u64>> { s.split('.').map(|x| x.parse().ok()).collect() };
    Some(p(a)?.cmp(&p(b)?))
}

/// Whether a dependency is present in an org (`package installed list` rows).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallState {
    Same,
    Older(String),
    Newer(String),
    Missing,
    Unknown,
}

/// Matches a dependency node against installed packages: by 04t first, then
/// by package name with a numeric version comparison.
pub fn install_state(node: &Node, installed: &[serde_json::Value]) -> InstallState {
    let s =
        |r: &serde_json::Value, k: &str| r.get(k).and_then(|v| v.as_str()).unwrap_or("").to_owned();
    if installed
        .iter()
        .any(|r| s(r, "SubscriberPackageVersionId") == node.id)
    {
        return InstallState::Same;
    }
    let (name, version) = split_label(&node.label);
    let Some(row) = installed
        .iter()
        .find(|r| s(r, "SubscriberPackageName") == name)
    else {
        return InstallState::Missing;
    };
    let have = s(row, "SubscriberPackageVersionNumber");
    match cmp_versions(&have, version) {
        Some(std::cmp::Ordering::Equal) => InstallState::Same,
        Some(std::cmp::Ordering::Less) => InstallState::Older(have),
        Some(std::cmp::Ordering::Greater) => InstallState::Newer(have),
        None => InstallState::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Shapes copied from the CLI source's producers (see module docs).
    const ANCESTRY: &str = "strict graph G {\n\t node04tA0000000000001 [label=\"1.0.0.1\"]\n\t node04tA0000000000002 [label=\"1.1.0.3\"]\n\t node04tA0000000000004 [label=\"1.1.1.1\"]\n\t node04tA0000000000003 [label=\"1.2.0.1\"]\n\t node04tB0000000000001 [label=\"2.0.0.1\"]\n\t node04tA0000000000001 -- node04tA0000000000002\n\t node04tA0000000000001 -- node04tA0000000000004\n\t node04tA0000000000002 -- node04tA0000000000003\n}";

    const DEPS: &str = "strict digraph G {\n\t node_04tBASE0000000001 [label=\"Base@2.3.0.1\"]\n\t node_04tUTIL0000000001 [label=\"Utils@1.4.0.2\" color=\"green\"]\n\t node_04tAPP00000000001 [label=\"App@1.2.0.1\" color=\"green\"]\n\t node_04tBASE0000000001 -> node_04tUTIL0000000001\n\t node_04tUTIL0000000001 -> node_04tAPP00000000001\n\t node_04tBASE0000000001 -> node_04tAPP00000000001\n}";

    #[test]
    fn ancestry_forest_with_two_roots() {
        let g = parse_dot(ANCESTRY).unwrap();
        assert!(!g.directed);
        assert_eq!(g.nodes.len(), 5);
        let lines: Vec<String> = g
            .forest(false)
            .iter()
            .map(|l| format!("{}{}", l.prefix, g.node(&l.id).unwrap().label))
            .collect();
        assert_eq!(
            lines,
            [
                "1.0.0.1",
                "├─ 1.1.0.3",
                "│  └─ 1.2.0.1",
                "└─ 1.1.1.1",
                "2.0.0.1"
            ]
        );
        assert_eq!(
            g.path_to_root("04tA0000000000003"),
            [
                "04tA0000000000003",
                "04tA0000000000002",
                "04tA0000000000001"
            ]
        );
        assert_eq!(g.child_count("04tA0000000000001"), 2);
        assert!(g.path_to_root("04tMISSING").is_empty());
    }

    #[test]
    fn dependency_install_order_and_tree() {
        let g = parse_dot(DEPS).unwrap();
        assert!(g.directed);
        let order: Vec<&str> = g
            .install_order()
            .unwrap()
            .iter()
            .map(|n| n.label.as_str())
            .collect();
        assert_eq!(order, ["Base@2.3.0.1", "Utils@1.4.0.2", "App@1.2.0.1"]);
        assert!(g.node("04tAPP00000000001").unwrap().highlighted);
        assert!(!g.node("04tBASE0000000001").unwrap().highlighted);
        // Tree from the selected package down to what it needs.
        let tree: Vec<String> = g
            .forest(true)
            .iter()
            .map(|l| format!("{}{}", l.prefix, g.node(&l.id).unwrap().label))
            .collect();
        assert_eq!(
            tree,
            [
                "App@1.2.0.1",
                "├─ Utils@1.4.0.2",
                "│  └─ Base@2.3.0.1",
                "└─ Base@2.3.0.1"
            ]
        );
    }

    #[test]
    fn being_built_and_cycles_and_garbage() {
        let g = parse_dot("strict digraph G {\n\t node_VERSION_BEING_BUILT [label=\"App@VERSION_BEING_BUILT\" color=\"green\"]\n}").unwrap();
        assert_eq!(
            split_label(&g.nodes[0].label),
            ("App", "VERSION_BEING_BUILT")
        );
        let cyc = parse_dot("strict digraph G {\n node_a -> node_b\n node_b -> node_a\n}").unwrap();
        assert_eq!(cyc.install_order(), Err(GraphError::Cycle));
        assert!(parse_dot("digraph X {}").is_err());
        assert!(parse_dot("strict graph G {\n what is this\n}").is_err());
    }

    #[test]
    fn install_states() {
        let g = parse_dot(DEPS).unwrap();
        let installed = vec![
            json!({"SubscriberPackageVersionId": "04tBASE0000000001", "SubscriberPackageName": "Base", "SubscriberPackageVersionNumber": "2.3.0.1"}),
            json!({"SubscriberPackageVersionId": "04tOTHER", "SubscriberPackageName": "Utils", "SubscriberPackageVersionNumber": "1.3.0.5"}),
        ];
        let st = |id: &str| install_state(g.node(id).unwrap(), &installed);
        assert_eq!(st("04tBASE0000000001"), InstallState::Same);
        assert_eq!(
            st("04tUTIL0000000001"),
            InstallState::Older("1.3.0.5".into())
        );
        assert_eq!(st("04tAPP00000000001"), InstallState::Missing);
        assert_eq!(
            cmp_versions("1.10.0.1", "1.2.0.9"),
            Some(std::cmp::Ordering::Greater)
        );
        assert_eq!(split_label("App@1.2.0.1 (04tX)"), ("App", "1.2.0.1"));
    }
}
