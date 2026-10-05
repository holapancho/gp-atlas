# Idea: Ancestry & Dependencies views

Status: **built in `gp-atlas-tui`** (2026-10-05), except the installation-key
prompt (§3, "Key-protected versions") and the desktop app. See "Implementation
notes" at the end for what differs from this draft.

Show, for any 2GP package version, **where it comes from** (ancestry) and
**what it needs** (dependencies), directly in the 2GP tab, read-only, using
the two `sf` commands GP Atlas already allows:

| View | Command | Already in allow-list |
|---|---|---|
| Ancestry | `sf package version displayancestry --package <0Ho|04t|alias>` | ✅ `PkgVersionAncestry` |
| Dependencies | `sf package version displaydependencies --package <04t|08c|alias>` | ✅ `PkgVersionDeps` |

Sources: the Salesforce pages the owner linked (CLI reference for
`displaydependencies` and `displayancestry`, and the 2GP guides "Package
ancestors" and "View package ancestors"). They could not be fetched from the
build environment (network policy), so the facts below come from the same
text bundled in the installed CLI (`plugin-packaging` 3.0.6 `messages/`), the
library code (`@salesforce/packaging` 5.0.7), and the plugin's real-org tests.
**Re-check against those pages before building.**

---

## 1. Why

- "Which version is this one built on?" and "Can I still upgrade from 1.2 to
  1.5?" are ancestry questions. Today you read a wall of DOT text or a
  terminal tree.
- "What must be installed first?" is a dependency question. The CLI answers it
  only as Graphviz code you have to render somewhere else.
- Both are needed right before an install or promote, which is exactly when
  the user is already looking at the version in GP Atlas.

## 2. What the CLI provides (facts)

### Ancestry (`displayancestry`)

- **Only second-generation managed packages.** Unlocked → error
  `unlockedPackageError` ("Package ancestry is available only for
  second-generation managed packages").
- `--package 0Ho…` (or package alias) → the tree of **every released version**
  of the package. `--package 04t…` → the tree of that version.
- Only **released** versions are included (`IsReleased = true`). No released
  version → `noVersionsError`.
- Built from `Package2Version.AncestorId`: each version points to the version
  it was built on; roots have `AncestorId = NULL`.
- `--json` without `--dot-code` returns a **tree**: `{ data, children[] }`,
  where `data` has `SubscriberPackageVersionId, MajorVersion, MinorVersion,
  PatchVersion, BuildNumber, depthCounter` (SPEC F30, verified by real-org
  tests). This can be drawn natively, no Graphviz needed.
- `--dot-code` returns `strict graph G { node<04t> [label="M.m.p.b"] … node<a> -- node<b> }`.
- `--verbose` adds the 04t next to the version number.
- Errors: `idOrAliasNotFound`, `versionNotFound`, `unlockedPackageError`,
  `noVersionsError`.

### Dependencies (`displaydependencies`)

- For **unlocked and 2GP managed** package versions.
- `--package` is a version (`04t…`), a version **create request** (`08c…`) or
  an alias. For an `08c` the version being built appears as a node labelled
  `<Package>@VERSION_BEING_BUILT`.
- **Precondition:** the version must have been built with
  `"calculateTransitiveDependencies": true` in `sfdx-project.json`. Otherwise
  `invalidDependencyGraphError` / `noDependencyGraphJsonMustProvideVersion`
  ("…set the value to true… you must create a new package version").
  The graph comes from `Package2VersionCreateRequest.DependencyGraphJson`.
- Output is **always DOT**, also in `--json` (result is the string):
  `strict digraph G { node_<04t> [label="Name@M.m.p.b" …]  node_<a> -> node_<b> }`.
- `--edge-direction root-first` (default): the root is the package installed
  **last**. `root-last`: edges point in **install order** (farthest leaf
  first, base package last).
- `--verbose`: label shows 04t and version number.
- `--installation-key` for key-protected versions. **This is a secret** (SPEC §10).

## 3. Proposed UX (TUI first, same design for the desktop app)

In **2GP Packages & Versions**, on a selected version:

| Key / click | Opens |
|---|---|
| `a` / "Ancestry" | Ancestry panel for this version (or `A`: whole package tree) |
| `d` / "Dependencies" | Dependencies panel for this version |

Disabled (with a reason in the footer) when not applicable (SPEC AC-25):
ancestry only for `ContainerOptions = Managed`; dependencies for Managed and
Unlocked.

### Ancestry panel

```
fake0221 — ancestry (released versions only)
└─ 1.0.0.1  04t…A1
   ├─ 1.1.0.3  04t…B2
   │  └─ 1.2.0.1  04t…C3   ◀ selected
   └─ 1.1.1.1  04t…D4      (patch branch)
```

- Native tree from the JSON (no DOT parsing). Selected version highlighted;
  its **path to the root** shown bold.
- Header line: "Built on 1.1.0.3 · 1 version built on this one".
- Actions: `c` copy 04t of a node, `Enter` opens that version's details,
  `g` copy the `--dot-code` command (copy-only, to render elsewhere).
- Hints from the 2GP guide to surface as help text (verify wording):
  what the ancestor means for upgrades, and that ancestry is set at build
  time (`ancestorVersion` / `ancestorId` in `sfdx-project.json`), which GP
  Atlas never edits.

### Dependencies panel

Two views of the same graph, toggled with `t`:

1. **Install order** (root-last, the practical one):
   ```
   Install in this order into a target org:
   1. BasePkg@2.3.0.1        04t…    ✅ installed (2.3.0.1)
   2. Utils@1.4.0.2          04t…    ⚠ installed 1.3.0.5 (older)
   3. fake0221@1.2.0.1       04t…    ⛔ not installed   ◀ selected
   ```
2. **Tree**: who depends on whom (root-first).

- Parse the DOT text: node lines `node_<id> [label="Name@ver" …]`, edge lines
  `node_<a> -> node_<b>`; topological sort for the install order.
- **Cross-check with the selected org** (Installed tab data, same 04t /
  package): mark each dependency installed / older / missing. This turns the
  view into "can I install this version in org X?". Read-only; the install
  command stays copy-only (SPEC v1.1 templates).
- Actions: `c` copy 04t, `i` copy install link of a dependency, `g` copy the
  command that prints the DOT code.
- Error `invalidDependencyGraphError` → friendly state: "This version was
  built without `calculateTransitiveDependencies: true`. Rebuild with it to
  see dependencies." Show the `sfdx-project.json` snippet as copy-only text.

### Key-protected versions

- Ask for the installation key in a masked prompt only when the CLI says it
  is needed; keep it in memory for that one call; never store it.
- **Redact it from History**, the debug output and diagnostics (today History
  shows the full argv; it must show `--installation-key <redacted>`).

## 4. Implementation sketch

- `gp-atlas-core`:
  - `ancestry.rs`: lenient parser for the `{data, children}` tree; path to a
    node; children count. Tests on a recorded fixture.
  - `depgraph.rs`: small DOT parser for exactly the CLI's format (nodes,
    labels, `->` edges), topological order, cycle guard. Tests on fixtures.
  - Redaction of `--installation-key` values in `RunOutput.argv` for display.
- `gp-atlas-tui`: two panels in the 2GP tab (or a popup), jobs on the pool
  like other commands, caching per 04t (released versions never change, so a
  long TTL is safe).
- Desktop app (M4): same model, rendered as tree/list widgets.

## 5. Unknowns to capture before building (add to SPEC §4.7)

| ID | Unknown | How |
|---|---|---|
| U10 | Real `displaydependencies --json` output with and without `--verbose` (exact label format, extra node attributes, how the selected node is marked) | `gp-atlas-beta`/`sf` on a version built with `calculateTransitiveDependencies: true` |
| U11 | Error envelope when the version was built **without** transitive dependencies | Same, on an older version |
| U12 | Ancestry JSON for a **whole package** (`--package 0Ho`) with several roots / patch branches | Capture on a managed package with history |
| U13 | Behaviour with an `08c` still in progress (`VERSION_BEING_BUILT`) | Capture during a build |
| U14 | Installation-key flow: error name when the key is missing or wrong | Capture on a protected version (key never written to fixtures) |

## 6. Proposed acceptance criteria

- **AC-D1** Ancestry and Dependencies actions are enabled only for applicable
  package types and show the reason otherwise.
- **AC-D2** The ancestry tree for a version matches the CLI's JSON tree for a
  fixture, with the selected version and its path to the root highlighted.
- **AC-D3** Install order equals a topological sort of the CLI's DOT graph
  (root-last) for a fixture.
- **AC-D4** With an org selected, each dependency shows installed / older /
  missing based on `package installed list`.
- **AC-D5** The "no transitive dependencies" case shows the explanation and
  copy-only snippet, not a raw error.
- **AC-D6** An installation key never appears in History, logs, diagnostics or
  fixtures.

## 7. Out of scope

- Rendering images (PNG/SVG) of the graphs; copying the DOT command is enough.
- Changing ancestry or dependencies (they are set at build time in
  `sfdx-project.json`, which GP Atlas never edits).

---

## Implementation notes (2026-10-05)

- **Ancestry uses `--dot-code` for the whole package**, not the JSON tree:
  the JSON keeps only the first root, and for a 04t it returns descendants,
  not ancestors (SPEC F39). GP Atlas parses the DOT, draws every root, and
  computes the path from the selected version to its root itself.
- Keys on the 2GP tab: `a` ancestry (from a version: that version marked and
  its path to the root highlighted; from the packages pane: whole tree),
  `A` whole package tree, `d` dependencies of the selected version.
- Dependencies: install order (default) or tree (`t`), "(direct)" marks the
  CLI's highlighted direct dependencies, "In org" compares with the selected
  org's `package installed list` (by 04t, else by package name and numeric
  version). Unreleased versions get a note with their `AncestorVersion`.
- Not built yet: the installation-key prompt for key-protected versions
  (so no key ever reaches argv or History), and real captures U10–U14 —
  tests use DOT text shaped exactly like the CLI source produces.
- Code: `gp-atlas-core/src/graph.rs` (DOT parser, forest, path, install
  order, install state) and `gp-atlas-tui` (`GraphView`, jobs `Ancestry` /
  `Deps`).
