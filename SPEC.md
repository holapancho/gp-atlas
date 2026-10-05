# GP Atlas — Build Spec & Agent Prompt

> **Repo:** <https://github.com/holapancho/gp-atlas>
> **Target CLI:** Salesforce CLI (`sf`) **2.150.6 or newer** (decision 2026-10-05). **Baseline: 2.150.6**, which bundles `@salesforce/plugin-packaging` **3.0.6** (a new major; do not trust docs written for 2.x of that plugin). The manifest, facts and fixtures describe the baseline; newer versions are accepted command by command through the contract check (D4).
> **Status:** Spec v1. Every fact in §4 was verified on 2026-10-01 by installing `@salesforce/cli@2.150.6` and reading its own command metadata (`sf commands --json`), its plugin source, and real command output. Anything *not* verified is listed in §4.7 (Known unknowns) — do not guess those; capture a fixture.

---

## 0. How to use this file

This file is both the **prompt** and the **spec**. Put it in the repo root as `SPEC.md`, open your coding agent in the repo, and send the kickoff message in §16. The agent should read the whole file before writing code, then work milestone by milestone (§12).

---

## 1. Agent prompt (role, rules, working agreement)

You are a senior Rust engineer building **GP Atlas**: a small, cross-platform desktop app that lets a Salesforce developer **browse 2GP and 1GP package versions** and **build `sf` commands to run manually**, using the installed Salesforce CLI as its only data source.

**Hard rules — never violate these:**

1. **Read-only.** GP Atlas never executes a state-changing `sf` command. The runner accepts only a closed `ReadOnlyCommand` enum (§4.4). There is no API to run arbitrary arguments.
2. **`sf` 2.150.6 or newer.** The baseline is 2.150.6: the manifest, facts and fixtures come from it. Older versions are blocked with a clear message (§4.2). On a newer version, any allow-listed command whose flags differ from the baseline manifest is disabled individually (D4, `ContractDrift`); everything else runs. Behaviour differences found in newer versions are recorded as facts/fixtures for that version. Never invent a flag, command, JSON field or error code: use the manifest (§4.4) and fixtures; if unknown, add it to §4.7 and capture a real fixture.
3. **No shell.** Spawn `sf` with an argv vector (`Command::new(bin).args(..)`). Never build a command string for execution. Quoting exists only for the *copy-to-clipboard* feature (§9).
4. **Validate, don't escape.** Every user-supplied value is validated against a per-flag pattern before it can reach argv. Reject invalid values; do not try to sanitize them.
5. **No secrets.** Never call `sf org display`, never read auth files directly, never persist or log tokens or installation keys (§10).
6. **UI never blocks.** All `sf` calls run off the UI thread, have timeouts, are cancellable, and show progress (§7).
7. **Evidence over memory.** Tests use fixtures captured from the real CLI. Contract tests install exactly `@salesforce/cli@2.150.6` (baseline) and, non-blocking, the latest release (§11).
8. **Small, reviewable steps.** One milestone per PR. After each: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and the contract test must pass. Summarize what changed and which acceptance criteria (AC) were met.

**Interpretation notes (decisions already made — don't re-litigate):**

- UI toolkit: **egui/eframe** (native window, virtualized tables via `egui_extras`). All logic lives in a UI-free crate so a TUI frontend can be added later.
- Binary name: `gp-atlas`. Crate names: `gp-atlas-core`, `gp-atlas-egui`.
- License: `MIT`. README must say "Not affiliated with or endorsed by Salesforce." Do not use "Salesforce" or "sf" as part of the product name.

---

## 2. The idea

Salesforce developers constantly need to answer small packaging questions: *What is the latest released version of package X? What's its 04t ID and install link? Which version is on branch Y? What does the ancestry look like? What is installed in org Z and is it behind?* Today that means remembering `sf package version list` flags, aliases vs IDs, Dev Hub vs packaging org, and then reading wide terminal tables.

**GP Atlas** gives a fast visual map ("atlas") of **2GP** (Dev Hub: unlocked and managed 2GP) and **1GP** (packaging org) package versions, tells you up front **what you are permitted to see** in each org, and produces **exact, correctly-quoted `sf` commands** you copy and run yourself.

**Positioning:** a free, local, read-only companion. It is *not* a DevOps/CI platform. It makes no network calls of its own — everything goes through your installed `sf`.

---

## 3. Scope

### In scope (v1)

- CLI doctor: exact version gate, plugin-override detection, command/flag contract check.
- Org + alias discovery from `sf`.
- **Access Matrix**: per-org capability probes for `sf package` and `sf package1` read commands.
- 2GP: list packages, list/filter/sort versions, version detail, ancestry, dependencies, "latest released per package" (computed client-side).
- 1GP: list versions for a packaging org; version detail.
- Installed packages for an org.
- Version-creation requests (read-only monitoring).
- Alias handling: org aliases, project package aliases, deprecated-flag-alias avoidance (§6).
- Command Builder with safe copy for bash/zsh, PowerShell, cmd.

### Out of scope (v1)

- Executing `package version create/promote/delete/update`, `package create/delete/update`, `install/uninstall`, `convert`, `retrieve`, `push-upgrade schedule/abort`, `org login/logout`, `alias set`, `config set`, `plugins install`.
- Package bundles (`package bundle ...` exist in 2.150.6 but are hidden/beta — ignore).
- Editing `sfdx-project.json`.
- Any direct Salesforce API calls or token handling.

### v1.1 (optional, after v1 is green)

- **Copy-only** templates (never executed) for `package install`, `package version promote`, `package version create`, `package version delete`, each with a "⚠ changes state" badge. Flags come from the manifest. `--no-prompt` is never added automatically.
- "Outdated packages" view: `package installed list` for an org vs. latest released version per package in the Dev Hub.
- `package push-upgrade list`, `package1 version create get` (read-only status).

---

## 4. Platform contract: `sf` 2.150.6 (baseline) or newer

### 4.1 Install

```bash
# Node >= 22 is required by @salesforce/cli@2.150.6 (npm "engines" field)
npm install --global @salesforce/cli@latest   # any version >= 2.150.6
npm install --global @salesforce/cli@2.150.6  # the baseline (manifest regeneration, contract tests)
sf version --json        # -> "cliVersion": "@salesforce/cli/2.150.6"
```

Binary resolution order: `GP_ATLAS_SF_BIN` env var → setting in app config → `which sf` (use the `which` crate so Windows resolves `sf.cmd`).

### 4.2 Version gate (minimum 2.150.6)

- `sf version --json` → `cliVersion` must be **≥ 2.150.6**, compared numerically as `major.minor.patch` (a pre-release such as `2.150.6-rc.1` is older than `2.150.6`).
- Older or unparsable → state `CliVersionTooOld { found }`. **All `sf` features are disabled** except Doctor, with a copyable fix: `npm install --global @salesforce/cli@latest`.
- Newer than the baseline → run D4 against the live `sf commands --json`; commands with drift are disabled and listed, the rest work normally.
- Developer-only override (for an **older** version): `--allow-cli-version <ver>` flag or `GP_ATLAS_ALLOW_CLI_VERSION`. When active, show a persistent red banner "Unsupported CLI version — results may be wrong".

### 4.3 Verified facts (clean install, linux-x64, Node 22.22.2)

| # | Fact | How verified |
|---|------|--------------|
| F1 | `sf version --json` → `cliVersion: "@salesforce/cli/2.150.6"`. | Ran it. |
| F2 | Bundled core plugins include `packaging 3.0.6`, `org 6.0.11`, `data 5.1.7`, `auth 5.0.6`, `info 4.0.9`. | `sf plugins --core`. |
| F3 | `sf plugins --json` and `sf plugins --core --json` print a **bare JSON array** (no `{status,result}` envelope). Entries have `name`, `version`, `type`. Observed on a clean install: 29 × `core`, 10 × `jit` (on-demand plugins not yet installed). See F31 for `user`/`link`/`dev`. `sf plugins --json` also lists the root `@salesforce/cli` and all core plugins. | Ran both. |
| F4 | `sf commands --json` returns a bare array of **273** command objects (`id`, `aliases`, `flags{name→{type,char,required,options,default,aliases,deprecated…}}`, `summary`, `state`…). ~1.3 MB, ~3.5 s. | Ran it. |
| F5 | Timings (test machine): `sf version --json` ≈ 1.0 s; any single command ≈ 2.5 s+. Process startup dominates → cache, limit concurrency, show progress. | `time`. |
| F6 | Success envelope for envelope-style commands: `{"status":0,"result":…,"warnings":[…]}` (observed for `org list`, `alias list`, `config get`). | Ran them. |
| F7 | Error envelope (observed for `NamedOrgNotFoundError`): `{name,message,exitCode:2,context,cause,warnings,code,status:2,commandName,stack}` printed on **stdout**; process exit code 2. `message` for unknown org: "Parsing --target-dev-hub … No authorization information found for <x>." | Ran `sf package list -v nobody@example.com --json` and `sf package1 version list -o … --json`. |
| F8 | `sf org list --json` → `result` has exactly the keys `other, sandboxes, nonScratchOrgs, devHubs, scratchOrgs`. A `warnings` entry says secrets are hidden. Flags: `--all`, `--skip-connection-status`, `--verbose`, `--clean` (**never use `--clean`**). | Ran it; read flags. |
| F9 | Package commands query the **Tooling API** (`Package2`, `Package2Version`, `Package2VersionCreateRequest`, `InstalledSubscriberPackage`, `SubscriberPackageVersion`, `MetadataPackageVersion`). | Plugin/library source. |
| F10 | `package version list` and `package1 version list` return up to **10,000** rows; more requires env `SF_ORG_MAX_QUERY_LIMIT`. | Library source comment + release notes. |
| F11 | `package list` **hides deprecated packages** (`IsDeprecated=false`). | Plugin source. |
| F12 | `package list` returns `Alias: ""` for every row (project aliases are **not** resolved by this command in 2.150.6). | Plugin source (`mapRecordsToResults` called without a project). |
| F13 | `package version list` fills `Alias` only when the process cwd is inside an sfdx project; `--packages` accepts project aliases **only in a project**, otherwise values must be 0Ho IDs (else error "invalid package id"). | Plugin + library source (`maybeGetProject`, `getPackageIdFromAlias`). |
| F14 | `--order-by` is pasted **raw** into SOQL `ORDER BY …`; `--branch` is pasted into SOQL inside single quotes. → The app must allow-list/validate these values. | Library source (`assembleQueryParts`, `constructWhere`). |
| F15 | Default sort: `Package2Id, Branch, MajorVersion, MinorVersion, PatchVersion, BuildNumber`. Version string = `Major.Minor.Patch.Build`. | Library source. |
| F16 | `--created-last-days` / `--modified-last-days` are integers ≥ 0 (`0` = today). | Library source (`validateDays`). |
| F17 | In `--json` mode `IsReleased` and `IsPasswordProtected` are booleans; in table mode they are strings. Dates are strings `YYYY-MM-DD HH:mm` (UTC, from ISO). `ReleaseVersion` is a one-decimal string. | Plugin source. |
| F18 | `InstallUrl` = `https://login.salesforce.com/packaging/installPackage.apexp?p0=<04t>` (field present in `package version list` rows). | Library source (`INSTALL_URL_BASE`). |
| F19 | Flag aliases such as `--targetdevhubusername`, `--targetusername`, `-u`, `--orderby`, `--createdlastdays` are **deprecated** (`deprecateAliases: true`). Canonical names are `--target-dev-hub`, `--target-org`, `--order-by`, `--created-last-days`. `--loglevel` is deprecated/ignored. | `sf commands --json`. |
| F20 | If a packaging object isn't available in the target org, the CLI maps `INVALID_TYPE … sObject type '…Package…' is not supported` (and a specific `NOT_FOUND`) to an action: *"Packaging is not enabled on this org…"*. | Library source (`applyErrorAction`). |
| F21 | `package version displaydependencies --json` returns a **DOT (GraphViz) string**. `package version displayancestry --dot-code --json` returns a DOT string; without `--dot-code` it returns an ancestry JSON structure. | Plugin source. |
| F22 | `package version retrieve` references a `DownloadPackageVersionZips` user permission; `package push-upgrade schedule` docs reference "Create and Update Second-Generation Packages". Neither is needed for the read-only commands in v1; **do not hardcode permission names as gates for listing** — probes (§5) are the source of truth. | Plugin messages. |
| F23 | The CLI's own docs warn that a user-installed `@salesforce/plugin-packaging` overrides the bundled one and stops following CLI updates. → Detect overrides (§5.2). | Plugin README. |
| F24 | `package1 version list --package-id` accepts **only 18-character** 033 IDs (`Flags.salesforceId({ length: 18, startsWith: '033' })`; error "The id must be 18 characters."). `package1 version display --package-version-id` (04t) and `package1 version create get --request-id` (0HD) accept 15 or 18. This is **not** visible in `sf commands --json`, so the manifest cannot catch it. | Installed plugin source (`lib/commands/package1/version/{list,display,create/get}.js`); plugin NUT `versionList.nut.ts`. |
| F25 | `org list --json` always strips `refreshToken` and `clientSecret`, but **replaces** `accessToken` (and `password` when present) with the text `"[REDACTED] Use 'sf org auth show-…' to view"`. If the env var **`SF_TEMP_SHOW_SECRETS=true`** is set, the **real** access token and password are returned. In plugin-org 6.0.11 the JSON always carries a warning about this (6.0.18+ only warns when the env var is set) — never use the warning as a signal. | plugin-org 6.0.11 `src/shared/orgListUtil.ts`, `src/commands/org/list.ts`. |
| F26 | `org list` groups: `devHubs`, `sandboxes`, `other` are **subsets of `nonScratchOrgs`** (same objects appear twice) → de-duplicate by `username`. `defaultMarker` is `"(D)"`, `"(U)"` or `"(D),(U)"`. `connectedStatus` is `"Connected"` for a healthy org and is **absent** with `--skip-connection-status`. `isDevHub` is checked live only for connected orgs; with `--skip-connection-status` it relies on cached auth data (can miss hubs). Expired/deleted scratch orgs are hidden without `--all`. | plugin-org 6.0.11 source + NUT `listAndDisplay.nut.ts`; schema `schemas/org-list.json` (field list). |
| F27 | `package version list` row details (JSON mode): `CreatedBy` is the **user Id** (`005…`), not a name. `CodeCoverage` is the literal `"use --verbose for code coverage"` without `--verbose`; with it `"<n>%"`, `"N/A"` (org-dependent or validation skipped) or `""`. `HasPassedCodeCoverageCheck` is a **bool or `"N/A"`**. `AncestorId`/`AncestorVersion` are `"N/A"` for unlocked packages and **absent** for a managed version without ancestor. `IsOrgDependent` is `"N/A"` for managed. `HasMetadataRemoved` is `"N/A"` for unlocked. `HasVpi` is the string `"true"`/`"false"`/`"N/A"`, or absent (needs `--verbose` and API ≥ 67). `Language` absent without `--verbose`. `--concise` only changes table columns, not JSON. Empty result: `result: []` + warning `"No results found"`, exit 0. | Installed `lib/commands/package/version/list.js` (identical in 3.0.6 and 3.0.8 source); NUT `packageVersion.nut.ts` asserts the exact key sets. |
| F28 | `package list` JSON keys: `Id, SubscriberPackageId, Name, Description, NamespacePrefix, ContainerOptions, ConvertedFromPackageId, PackageErrorUsername, Alias, AppAnalyticsEnabled, CreatedBy, IsOrgDependent`; `AppAnalyticsEnabled` absent below API 59. Real capture: `--verbose` produces **byte-identical JSON** (it only adds table columns); `AppAnalyticsEnabled` is a boolean; `ConvertedFromPackageId` and `PackageErrorUsername` are `null` (not `""`) when unset; `CreatedBy` is a user Id (`005…`); `Alias` is `""` (F12); `IsOrgDependent` is `"N/A"` for managed, `"Yes"`/`"No"` for unlocked. | NUT `packageList.nut.ts`; owner's capture `fixtures/sf-2.150.6/package-list.real.json`. |
| F29 | `package version create list` keys: `Id, Status, Package2Id, Package2Name, Package2VersionId, SubscriberPackageVersionId, Tag, Branch, Error, CreatedDate, HasMetadataRemoved, HasPassedCodeCoverageCheck, CreatedBy, ConvertedFromVersionId, CodeCoverage, VersionNumber`; `--verbose` adds `VersionName`. `create report` returns an **array** with the same keys plus `TotalNumberOfMetadataFiles`, `TotalSizeOfMetadataFiles`. | NUT `packageVersion.nut.ts`; schemas `package-version-create-{list,report}.json`. |
| F30 | `displayancestry --json` (no `--dot-code`): `{ data, children[] }` recursively; `data` has **only** `SubscriberPackageVersionId, MajorVersion, MinorVersion, PatchVersion, BuildNumber, depthCounter` (the schema also lists `AncestorId`, but real output omits it). With `--dot-code` the result is a string starting `strict graph G {`; `displaydependencies` returns a string starting `strict digraph G {`. | NUT `packageVersion.nut.ts`; unit test `displayDependencies.test.ts`. |
| F31 | `sf plugins --json` `type` values: `core`, `user` (`plugins install`), `link` (`plugins link`), `dev`, `jit`. User plugins load **before** core ones and a same-named later plugin is skipped, so an override of `@salesforce/plugin-packaging` appears as a single entry with `type: "user"` (or `"link"`). | Installed `@oclif/plugin-plugins` 5.5.1 `lib/commands/plugins/index.js`; `@oclif/core` `lib/config/plugin-loader.js`. |
| F32 | The CLI finds `sfdx-project.json` by searching the cwd and **every parent directory** up to the filesystem root. | Installed `@salesforce/core` `lib/util/internal.js` (`traverse.forFile`). |
| F33 | `package1 version list`/`display` JSON: `BuildNumber` is a **number**; other fields strings. Empty result: exit 0, `result: []`, warning `"No Results Found"` (list) / `"No results found"` (display). The 1GP query (`MetadataPackageVersion`, Tooling) has **no** client-side error mapping (F20 does not apply to 1GP). | Installed `@salesforce/packaging` 5.0.7 `lib/package1/package1Version.js`; plugin NUTs. |
| F34 | `package1 version create get --json` returns the `PackageUploadRequest` record, which can contain **`Password`** (the 1GP installation key). `Status` is `QUEUED`/`IN_PROGRESS`/`SUCCESS`/`ERROR`; on `ERROR` the command **fails** (`uploadFailure`) instead of returning the record. | Installed `lib/commands/package1/version/create/get.js`; schema `package1-version-create-get.json`. |
| F35 | `@salesforce/plugin-packaging` 3.0.6 ships **JSON Schemas** for every command result in `schemas/` (inside the installed CLI). They describe types, not runtime behaviour, and can be wrong in details (F30). | Installed package. |
| F36 | Real `org list --json` (48 orgs): every org object has `username, accessToken (redacted text), instanceUrl, orgId, loginUrl, clientId, instanceApiVersion, instanceApiVersionLastRetrieved (locale string, e.g. "9/30/2026, 4:08:45 PM"), isDevHub, namespacePrefix (string or null), name, instanceName, isSandbox, isScratch, trailExpirationDate (ISO "…+0000" or null), orgEdition, alias (may be null), isDefaultDevHubUsername, isDefaultUsername, lastUsed (ISO)`; `tracksSource` is sometimes absent. Non-scratch orgs add `connectedStatus` and (only when default) `defaultMarker`. **`connectedStatus` is free text**: `"Connected"` or an error message, observed: `"Session expired or invalid"`, `"Unable to refresh session due to: Error authenticating with the refresh token due to: authentication failure"`, and a multi-line HTML/HTTP 420 error. Scratch orgs have **no** `connectedStatus`; they add `devHubUsername, created (epoch ms as a string), expirationDate ("YYYY-MM-DD"), createdOrgInstance, signupUsername, createdBy (a username), createdDate (ISO), devHubOrgId, devHubId, attributes, orgName, edition, status ("Active"), isExpired, namespace`. `loginUrl` may or may not have a trailing `/`. | Owner's capture `fixtures/sf-2.150.6/org-list.real.json`. |
| F37 | `@salesforce/cli` **2.152.14** (latest on 2026-10-05) bundles `@salesforce/plugin-packaging` **3.0.7**; all 18 manifest commands match the 2.150.6 baseline (no D4 drift). | Contract tests in compat mode against a clean install. |
| F38 | An org that no longer resolves (deleted scratch org, expired trial, refreshed sandbox, changed domain) makes data commands fail with `name: "ERROR_HTTP_420"`, `code: "1"`, `exitCode: 1`, message "HTTP response contains html content. Check that the org exists and can be reached. HTTP status code: 420…"; `data`, `stack` and `cause` are printed as `"hidden"`. `org list` shows the same text as `connectedStatus` (F36). Classified `Unreachable(OrgUnavailable)`. | Owner's capture `fixtures/sf-2.150.6/package-installed-list.http-420.json`. |
| F39 | `displayancestry --json` (no `--dot-code`) returns only `children[0]` of the producer, i.e. **the first root** for a package (`0Ho`) with several roots; for a version (`04t`) it returns that version and its **descendants**, not its ancestors. `--dot-code` lists **all** released versions of all roots with parent→child edges (`node<04t> [label="M.m.p.b"]`, `node<a> -- node<b>`). `displaydependencies` always returns DOT (`node_<04t> [label="Name@M.m.p.b"( color="green")]`, green = selected version and its direct dependencies; root-last edges `node_<a> -> node_<b>` = install a before b). GP Atlas therefore uses `--dot-code` for ancestry and computes the path to the root itself. | Installed `@salesforce/packaging` 5.0.7 `lib/package/packageAncestry.js`, `packageVersionDependency.js`. |

### 4.4 Command manifest (allow-list)

`*` = required. IDs: `0Ho` package, `04t` subscriber package version, `08c` version-create request, `033` 1GP metadata package, `0HD` 1GP upload request. ID rule (CLI `validateId`): length 15 or 18 and correct prefix. **Exception:** `package1 version list --package-id` requires exactly 18 characters (F24).

**Runnable (read-only) — implement as `ReadOnlyCommand` variants:**

| Variant | Command | Target | Flags (canonical long names) |
|---|---|---|---|
| `Version` | `sf version --json` | — | — |
| `Plugins` | `sf plugins --json` | — | — |
| `Commands` | `sf commands --json` | — | — |
| `OrgList` | `sf org list --json` | — | `--skip-connection-status`, `--all` |
| `AliasList` | `sf alias list --json` | — | — |
| `PkgList` | `sf package list` | Dev Hub | `--target-dev-hub`*, `--verbose`, `--api-version` |
| `PkgVersionList` | `sf package version list` | Dev Hub | `--target-dev-hub`*, `--packages` (CSV of 0Ho or project aliases), `--released`, `--branch`, `--created-last-days <int≥0>`, `--modified-last-days <int≥0>`, `--order-by`, `--concise`, `--verbose`, `--show-conversions-only`, `--api-version` |
| `PkgVersionReport` | `sf package version report` | Dev Hub | `--target-dev-hub`*, `--package`* (04t or alias), `--verbose` |
| `PkgVersionAncestry` | `sf package version displayancestry` | Dev Hub | `--target-dev-hub`*, `--package`* (0Ho/04t/alias), `--dot-code`, `--verbose` |
| `PkgVersionDeps` | `sf package version displaydependencies` | Dev Hub | `--target-dev-hub`*, `--package`* (04t/08c/alias), `--edge-direction root-first\|root-last` (default root-first), `--installation-key` (secret, §10), `--verbose` |
| `PkgCreateList` | `sf package version create list` | Dev Hub | `--target-dev-hub`*, `--created-last-days`, `--status Queued\|InProgress\|Success\|Error`, `--show-conversions-only`, `--verbose` |
| `PkgCreateReport` | `sf package version create report` | Dev Hub | `--target-dev-hub`*, `--package-create-request-id`* (08c) |
| `PkgInstalledList` | `sf package installed list` | Any org | `--target-org`* |
| `Pkg1VersionList` | `sf package1 version list` | Packaging org | `--target-org`*, `--package-id` (033; omit = all packages in org) |
| `Pkg1VersionDisplay` | `sf package1 version display` | Org | `--target-org`*, `--package-version-id`* (04t) |

All runnable commands are invoked with `--json` by the runner. The app always passes the Dev Hub / org explicitly (never relies on `target-dev-hub`/`target-org` config defaults).

**Optional v1.1 runnable:** `package push-upgrade list` (`--target-dev-hub`*, `--package`* (033), `--scheduled-last-days`, `--status`, `--show-push-migrations-only`), `package1 version create get` (`--target-org`*, `--request-id`* 0HD).

**Never runnable (blocklist, enforced by the enum's absence and by a test that greps for them):** anything ending in `create`, `delete`, `promote`, `update`, `install`, `uninstall`, `convert`, `retrieve`, `schedule`, `abort`; `org display`, `org login`, `org logout`, `org list --clean`, `config set`, `alias set`, `plugins install/link/uninstall/update`.

**Safe `--order-by` allow-list** (all are fields queried by `package version list`; add `ASC`/`DESC` via a separate control): `CreatedDate, LastModifiedDate, MajorVersion, MinorVersion, PatchVersion, BuildNumber, Package2Id, Branch, Name, IsReleased`. Multiple fields allowed, comma-separated, each from this list. *(Verify each against a live Dev Hub in an integration test.)*

The committed file `manifest/sf-2.150.6.json` is **generated** from `sf commands --json` by `tools/extract-manifest` for exactly the commands above (id, flag name, type, char, required, options, default). Code reads this manifest to (a) render builder forms and (b) run the contract check (§5.2). Never hand-edit it.

### 4.5 Result models (`--json`)

Use lenient serde: `#[serde(default)]`, `Option<T>`, **no `deny_unknown_fields`**, and tolerate number-or-string fields (e.g. `BuildDurationInSeconds` is a number or `""`).

Exact key sets and value rules per command are in F27–F30 and F33–F34; the lists below are the union of fields.

**`PackageRow`** (`package list`): `Id` (0Ho), `SubscriberPackageId` (033), `Name`, `Description`, `NamespacePrefix`, `ContainerOptions` (`Managed`|`Unlocked`), `ConvertedFromPackageId`, `Alias` (always `""`, F12), `IsOrgDependent` (`Yes`|`No`|`N/A`), `PackageErrorUsername`, `AppAnalyticsEnabled`, `CreatedBy`.

**`PackageVersionRow`** (`package version list`): `Package2Id`, `Branch`, `Tag`, `MajorVersion`, `MinorVersion`, `PatchVersion`, `BuildNumber`, `Id` (Package2Version id), `SubscriberPackageVersionId` (04t), `ConvertedFromVersionId`, `Name`, `NamespacePrefix`, `Package2Name`, `Description`, `Version` (`M.m.p.b`), `IsPasswordProtected` (bool), `IsReleased` (bool), `CreatedDate`, `LastModifiedDate` (`YYYY-MM-DD HH:mm`), `InstallUrl`, `CodeCoverage` (string; meaningful only with `--verbose`), `HasPassedCodeCoverageCheck`, `ValidationSkipped`, `ValidatedAsync`, `AncestorId`, `AncestorVersion`, `Alias` (CSV; blank outside a project, F13), `IsOrgDependent` (`Yes`|`No`|`N/A` — `N/A` for managed), `ReleaseVersion`, `BuildDurationInSeconds`, `HasMetadataRemoved` (`Yes`|`No`|`N/A`), `CreatedBy` (user Id `005…`, F27), `Language`, `HasVpi` (`"true"`|`"false"`|`"N/A"`|absent). Mixed types: `HasPassedCodeCoverageCheck` (bool or `"N/A"`), `BuildDurationInSeconds` (number or `""`). Absent keys: `AncestorId`/`AncestorVersion` (managed without ancestor), `Language`/`HasVpi`/`HasPassedCodeCoverageCheck` (without `--verbose`).

**`InstalledPackageRow`** (`package installed list`): `Id`, `SubscriberPackageId`, `SubscriberPackageName`, `SubscriberPackageNamespace`, `VersionSettings` (`namespace`|`packageId`|`""`), `SubscriberPackageVersionId`, `SubscriberPackageVersionName`, `SubscriberPackageVersionNumber`.

**`Package1VersionRow`** (`package1 version list` / `display`): `MetadataPackageVersionId` (04t), `MetadataPackageId` (033), `Name`, `ReleaseState`, `Version` (`major.minor.patch`), `BuildNumber` (number, F33).

**`OrgRow`** (`org list`, F25–F26): all optional except `username`, `orgId`, `instanceUrl`; notable: `alias`, `isDevHub`, `isSandbox`, `isScratch`, `connectedStatus`, `isDefaultUsername`, `isDefaultDevHubUsername`, `defaultMarker`, `lastUsed`, `instanceApiVersion`, `namespacePrefix`, `orgEdition`, `name`; scratch orgs add `expirationDate`, `isExpired`, `devHubUsername`, `status`, `namespace`, `orgName`. Never deserialize `accessToken`, `password`, `refreshToken`, `clientSecret` into the model (§10).

**`PackageAncestryNode`** (F30), **`CreateRequestRow`** (F29): see facts.

**Envelope types:** `SfSuccess<T> { status: 0, result: T, warnings: Vec<String> }`, `SfError { name, message, exitCode, status, code?, context?, commandName?, warnings, actions? }`. Parse stdout JSON **even when the exit code is non-zero**; treat stderr as diagnostics only. `plugins` output is a bare array (F3) — give it its own parser.

### 4.6 Performance rules

- Per-invocation startup is 1–3 s (F5). Run `Version`, `Plugins`, `Commands` once at startup (in parallel, max 3 concurrent `sf` processes overall), cache `Commands` keyed by CLI version.
- Up to 10,000 version rows: tables must be virtualized; parse JSON off the UI thread; cap captured stdout at 64 MB.
- Default per-command timeout 120 s (configurable); on timeout kill the child (and its process tree on Windows) and report `Unknown(timeout)`.

### 4.7 Known unknowns — capture a real fixture before relying on any of these

| ID | Unknown | Status (2026-10-03) | Action |
|----|---------|--------|--------|
| U1 | Field list of each org object in `org list --json`. | 🟢 Resolved by a real capture (F36). | — |
| U2 | JSON keys of `package version report --json`. | 🟢 Resolved from schema + unit tests (F35). | Real capture optional. |
| U3 | JSON keys of `create list` / `create report`. | 🟢 Resolved (F29). | — |
| U4 | JSON shape of `displayancestry` without `--dot-code`. | 🟢 Resolved (F30). | — |
| U5 | Exact `name`/`code` for: not a Dev Hub, expired session, insufficient access, API disabled, network failure. | 🔴 Open — produced by the server/network, not the plugin. Org gone/unreachable resolved (`ERROR_HTTP_420`, F38). `org list` already shows the **text** of expired/refresh-failure sessions (F36), not the error envelope of a failing command. | Capture (docs/FIXTURE_CAPTURE.md). Until then classify by substring (§5.3) and fall back to `Unknown` with the raw message. |
| U6 | `sf plugins --json` `type` values for non-core installs. | 🟢 Resolved (F31). | — |
| U7 | `package1 version list` in a subscriber / non-packaging org. | 🔴 Open, narrowed (F33): either `[]` + warning, or a raw server error (e.g. `INVALID_TYPE`). | Capture; handle both outcomes meanwhile. |
| U8 | Sandbox install-link host (`test.salesforce.com`). | 🔴 Open. | Verify in a sandbox; otherwise label the sandbox link "constructed". |
| U9 | Does the CLI find `sfdx-project.json` in parent directories? | 🟢 Resolved: yes, up to the filesystem root (F32). | — |

---

## 5. Permissions & capability checks

**Principle:** don't predict permissions — **probe** them with the CLI's own read-only commands and report exactly what happened. An empty list is **Allowed** (the CLI returns status 0 with a "No results found" warning).

### 5.1 States

`Allowed` · `Denied(reason)` · `NotApplicable(reason)` · `Unreachable(reason)` (auth/network) · `Unknown(reason)` (not tested / timeout) · `ContractDrift(details)` (flag missing from CLI metadata).

### 5.2 Checks, in order

**L0 — Machine (Doctor screen)**

| # | Check | Command | Pass condition |
|---|-------|---------|----------------|
| D1 | `sf` found and runnable | resolve binary; run `sf version --json` | exit 0, JSON parses |
| D2 | **CLI version ≥ 2.150.6** | `sf version --json` | `cliVersion` ≥ `2.150.6` (§4.2) |
| D3 | Packaging plugin is the bundled one | `sf plugins --json` | exactly one entry `name == "@salesforce/plugin-packaging"` with `type == "core"` (any version; a version other than the 3.0.6 baseline is noted and covered by D4). `user`/`link`/other type → `PackagingPluginOverridden` (F23, F31) |
| D4 | **Contract**: every allow-listed command and flag exists as the manifest says | `sf commands --json` vs `manifest/sf-2.150.6.json` | all ids present; for each flag: name, type, `char`, `required`, `options` equal. Mismatch → `ContractDrift` for that command only |

**L1 — Inventory**

| # | Check | Command |
|---|-------|---------|
| I1 | Orgs | `sf org list --json` (first with `--skip-connection-status` for instant paint; then again without to fill connection status) |
| I2 | Org aliases | `sf alias list --json` |
| I3 | Defaults (display only) | `sf config get target-dev-hub target-org --json` |

Candidates: `devHubs[]` are 2GP Dev Hub candidates. All non-scratch orgs are candidates for 1GP and installed-package probes. Scratch orgs can be probed for `PkgInstalledList` only.

**L2 — Per-org capability probes** (lazy: when an org is selected, or "Check all"; max 2 in flight)

| Capability | Probe (all `--json`, read-only) | Allowed when |
|---|---|---|
| `Pkg2.ListPackages` | `sf package list --target-dev-hub <org>` | `status == 0` |
| `Pkg2.ListVersions` | `sf package version list --target-dev-hub <org> --concise --created-last-days 0` | `status == 0` (empty result is fine) |
| `Pkg2.CreateRequests` | `sf package version create list --target-dev-hub <org> --created-last-days 0` | `status == 0` |
| `Pkg2.InstalledList` | `sf package installed list --target-org <org>` | `status == 0` |
| `Pkg1.ListVersions` | `sf package1 version list --target-org <org>` | `status == 0` |

If an org is not in `devHubs`, still allow the user to probe 2GP capabilities ("Try anyway") but mark the expected result `NotApplicable("not flagged as Dev Hub")` until the probe proves otherwise.

### 5.3 Error classification (first match wins; always keep and show the raw error)

| Rule | State | Fix hint (copy-only; never executed) |
|---|---|---|
| `name == "NamedOrgNotFoundError"` *(observed, F7)* | `Unreachable(NotAuthenticated)` | `sf org login web --alias <alias>` |
| `name == "ERROR_HTTP_420"` *(observed, F38)* | `Unreachable(OrgUnavailable)` | "Org deleted, expired or moved": `sf org login web --alias <alias>` if it still exists, else `sf org logout --target-org <alias>` |
| `name`/`message` contains `INVALID_TYPE` and `sObject type` and `Package`, **or** the CLI's action text says packaging is not enabled *(F20)* | `Denied(PackagingNotEnabled)` | "This org is not a Dev Hub / second-generation packaging isn't enabled, or the object isn't exposed here." |
| substring (case-insensitive) `INSUFFICIENT_ACCESS` | `Denied(InsufficientAccess)` | "Your user lacks access to this object. Ask an admin." |
| substring `API_DISABLED` / `API_CURRENTLY_DISABLED` | `Denied(ApiDisabled)` | "API access is disabled for this org/user." |
| substring `INVALID_SESSION_ID`, `expired`, `invalid_grant`, `RefreshToken` | `Unreachable(SessionExpired)` | re-login hint as above |
| substring `ENOTFOUND`, `ETIMEDOUT`, `ECONN`, `EAI_AGAIN` | `Unreachable(Network)` | "Check your connection / VPN." |
| child killed by timeout | `Unknown(Timeout)` | "Retry; increase timeout in Settings." |
| anything else | `Unknown(Unclassified)` | show raw `name` + `message` |

The substring rules (all but the first two) are **heuristics until U5 is captured**; replace them with exact matches from fixtures.

### 5.4 Access Matrix UI

Rows = orgs (alias — username, type badge). Columns = capabilities above. Cell = icon + short reason; click → drawer with: state, raw `name`/`message`, exit code, duration, the exact command run (copyable), and a **Copy diagnostics** button (redacts usernames/org IDs/instance URLs by default). Buttons: **Re-test** (cell/row/all). Results cached per `(cli version, org username, capability)` for 15 min; invalidated when the org list changes.

### 5.5 Gating

Each screen requires its capability for the *selected* org. If not `Allowed`: show the state, reason, and fix hint instead of a table, plus **Try anyway** (still read-only). A required-but-`ContractDrift` command is disabled with a clear message.

---

## 6. Aliases

*(Interpreted here as "aliases" in the UI — org aliases, package aliases, and CLI flag aliases.)*

### 6.1 Three alias namespaces

1. **Org aliases** (global, owned by the CLI). Source: `alias list --json` and the alias field in `org list --json`. Used for `--target-dev-hub` / `--target-org`. Display everywhere as `alias — username`. The builder emits the alias by default (toggle: username). GP Atlas never edits aliases (copy-only hint `sf alias set …`).
2. **Package aliases** (project-scoped): `packageAliases` in `sfdx-project.json`, mapping name → `0Ho…` (package) or `04t…` (version; commonly `Name@1.2.0-3`). **The CLI resolves them only when its cwd is inside the project (F13); `package list` never fills them (F12).** So GP Atlas must resolve aliases itself.
3. **CLI flag aliases** (deprecated spellings, F19). Never emitted; the manifest only contains canonical names. A unit test asserts no builder output contains `--targetdevhubusername`, `--targetusername`, ` -u `, `--orderby`, `--createdlastdays`, `--modifiedlastdays`, `--packageid`, etc.

### 6.2 Project context

- A **Project** chip in the top bar: "No project" / `<folder>`. User picks a folder; GP Atlas looks for `sfdx-project.json` there (and, pending U9, in ancestors) and shows the resolved path.
- Parse **read-only** with a lenient JSON reader; only `packageAliases` is used. Never write the file.
- Join aliases to rows by ID using the **first 15 characters** (case-sensitive) so 15- and 18-char IDs match.

### 6.3 Display rules

- Package rows and version rows show **alias chips** (all aliases for that ID; first in file order is "primary"). Tooltip shows the full ID.
- Warnings (non-blocking): **orphan alias** (alias → ID not present in the current Dev Hub results — possibly another Dev Hub), **unaliased ID** (row has no alias), **duplicate** (several aliases → same ID).
- If the CLI's own `Alias` column is non-empty, show it too and flag a mismatch with the project-derived value.

### 6.4 Builder rules

- Each reference field (`--package`, `--packages`) has a toggle **ID** (default; works from any directory) / **Alias** (requires the project directory as cwd).
- When any alias is used, the preview offers an optional prefix `cd "<project dir>" &&` (shell-appropriate) and a visible note "alias resolves only inside the project".
- Aliases with spaces or metacharacters are quoted per the selected shell (§9). `--packages` is comma-separated, so aliases containing `,` are refused in that field.
- Provide **Copy `packageAliases` snippet** for the selected rows (`"Name@1.2.0-3": "04t…"`). Copy-only.

---

## 7. Architecture

### 7.1 Workspace

```
gp-atlas/
├─ Cargo.toml                    # workspace
├─ SPEC.md
├─ manifest/sf-2.150.6.json      # GENERATED from `sf commands --json`; committed
├─ fixtures/sf-2.150.6/          # sanitized real outputs (one file per command/case)
├─ tools/
│  ├─ extract-manifest.*         # regenerates the manifest from an installed sf
│  └─ capture-fixtures.*         # runs every read-only command and sanitizes output
├─ tests/
│  ├─ contract/                  # runs only in CI job `sf-contract`
│  └─ fake-sf/                   # tiny executable that replays fixtures by argv
└─ crates/
   ├─ gp-atlas-core/             # NO UI dependencies
   │  └─ src/{runner,manifest,envelope,doctor,orgs,probes,classify,project,aliases,builder,quote,cache,models/}.rs
   └─ gp-atlas-egui/             # binary `gp-atlas`
```

### 7.2 Core API sketch

```rust
pub enum ReadOnlyCommand { Version, Plugins, Commands, OrgList{skip_status:bool, all:bool},
  AliasList, PkgList{hub:OrgRef, verbose:bool}, PkgVersionList(PkgVersionListArgs),
  PkgVersionReport{..}, PkgVersionAncestry{..}, PkgVersionDeps{..},
  PkgCreateList{..}, PkgCreateReport{..}, PkgInstalledList{org:OrgRef},
  Pkg1VersionList{org:OrgRef, package_id:Option<Id033>}, Pkg1VersionDisplay{..} }

impl ReadOnlyCommand { pub fn argv(&self, manifest:&Manifest) -> Result<Vec<String>, ValidationError>; }

pub struct SfRunner { bin: PathBuf, sem: Semaphore, cfg: RunnerCfg }
impl SfRunner {
  pub async fn run(&self, cmd: ReadOnlyCommand, cancel: CancelToken) -> Result<RunOutput, RunError>;
}
```

- `argv()` is the **only** place argv is built; it validates every value (§9.2) and emits only canonical flags from the manifest.
- Typed newtypes for IDs (`Id0Ho`, `Id04t`, `Id08c`, `Id033`, `Id0HD`) validate prefix and length 15/18 on construction.

### 7.3 Runner behaviour

- `Command::new(bin).args(argv)`, stdin = null, stdout/stderr piped, working dir = project dir **only** when the user chose alias mode for a run, else the app's own cwd.
- Environment: pass through the user's environment; additionally set `NO_COLOR=1`, `SF_SKIP_NEW_VERSION_CHECK=true`, `SF_AUTOUPDATE_DISABLE=true`, and **always remove `SF_TEMP_SHOW_SECRETS`** from the child environment (F25). Do **not** alter telemetry settings. Do not rely on env for correctness — parse stdout JSON only.
- Optional setting "Max rows" → `SF_ORG_MAX_QUERY_LIMIT` (F10) for that process only.
- Concurrency semaphore = 3. Timeout 120 s default. Cancellation kills the child. Output cap 64 MB.
- Windows: resolve `sf.cmd` via `which`; because argv values are validated against strict patterns (§9.2), no value can contain shell metacharacters even when a `.cmd` shim is used.

### 7.4 UI ↔ core

A background tokio runtime in its own thread. The UI sends `Request`s over a channel and receives `Event`s; each event calls `ctx.request_repaint()`. No `await` on the UI thread. State lives in one `AppState`; screens are pure functions of state.

### 7.5 Cache & settings

- Settings: JSON in the OS config dir (`directories` crate): sf binary override, timeout, max rows, default shell, theme, last project dir, column choices.
- Cache: JSON files in the OS cache dir keyed by `(cli_version, org_username, command, canonical-args-hash)`; TTL 10 min for lists, 15 min for probes; UI shows "cached N min ago ⟳". Never cache secrets (none are ever obtained).

---

## 8. UI spec

Top bar: **CLI pill** (✅ 2.150.6 / ⛔ mismatch), **Dev Hub selector**, **Packaging/target org selector**, **Project chip** (§6.2), **Refresh**, **Settings**. Left nav: Doctor · Access · 2GP Versions · 1GP Versions · Installed · Build Requests · Command Builder · History.

| Screen | Content |
|---|---|
| **Doctor** | D1–D4 checklist with state, details, copyable fix commands, **Re-run**. Shown first on launch if anything is not green. |
| **Access** | The Access Matrix (§5.4). |
| **2GP Versions** | *Left:* packages (Name, Namespace, Managed/Unlocked badge, Org-dependent, alias chips). *Right:* filter bar — Released only, Branch (dropdown from loaded data + free text validated), Created last N days, Modified last N days, Show conversions only, Concise/Verbose, Order by (allow-list + ASC/DESC). Virtualized table (Version, 04t, alias chips, Released, Branch, Ancestor, Created, Code coverage when verbose). Row actions: **Copy 04t**, **Copy install link** (production; sandbox variant per U8), **Copy `report` command**, **Ancestry**, **Dependencies** (disabled when not supported by package type: ancestry = 2GP managed only; dependencies = unlocked or 2GP managed), **Details** (runs `package version report --verbose`). A **Latest released per package** toggle, computed client-side by max `(Major,Minor,Patch,Build)` among `IsReleased` rows, labelled "computed by GP Atlas". |
| **1GP Versions** | Org selector (packaging org), optional `033` filter, table (Name, ReleaseState, Version, BuildNumber, 04t). Actions: copy 04t, copy `package1 version display` command, Details. |
| **Installed** | Org selector; table from `package installed list`; Version Settings column; copy 04t / command. (v1.1: Outdated column.) |
| **Build Requests** | `create list` with status + days filters; row → `create report`; copy commands. Read-only monitoring. |
| **Command Builder** | §9. |
| **History** | Last 200 commands the app ran: timestamp, command (copyable, no secrets), exit code, duration, "re-run". |

**State rules for every data screen:** Loading (spinner, elapsed time, **Cancel**) · Empty (CLI "No results found") · Denied (reason + fix, §5.5) · Error (raw message + Copy diagnostics) · Stale (cached banner with Refresh). Light/dark follows the OS. Keyboard: `Ctrl/Cmd+R` refresh, `Ctrl/Cmd+F` focus filter, `Ctrl/Cmd+C` copy selected row's primary ID, `Esc` closes drawers/cancels loading.

---

## 9. Command Builder & quoting

### 9.1 Behaviour

- Pick a command → form generated from the manifest: booleans → toggles, enums → dropdowns, IDs → text with prefix/length validation, integers ≥ 0, text with the patterns below.
- Live preview of the **exact** command. Controls: shell (**bash/zsh · PowerShell · cmd**), long vs short flags (default long), `--json` toggle (**default off for copy** — humans prefer tables; **always on for app-run**), org alias vs username, package ID vs alias (§6.4), single-line vs multi-line.
- Buttons: **Copy** and (for runnable commands) **Run in GP Atlas**. v1.1 copy-only templates carry the "⚠ changes state — GP Atlas will not run this" badge.
- Secrets (`--installation-key`) are masked, never stored, never written to History; the preview shows `<INSTALLATION_KEY>` unless the user explicitly toggles "include in copied command" for that single copy.

### 9.2 Validation patterns (reject, don't sanitize)

| Value | Rule |
|---|---|
| Org alias / username | `^[A-Za-z0-9._@+\-]{1,120}$` |
| Package alias | `^[A-Za-z0-9 _.@+\-:/]{1,200}$` (spaces allowed — CLI examples use aliases with spaces) |
| Branch | `^[A-Za-z0-9._\-/]{1,100}$` (**no quotes/spaces** — F14) |
| `--order-by` | each comma-separated token = `<AllowedField>` optionally `ASC`/`DESC` (§4.4) |
| IDs | prefix + length 15 or 18 + `[A-Za-z0-9]` |
| Days | integer ≥ 0 |
| `--api-version` | `^\d{2}\.\d$` |

### 9.3 Quoting (copy only)

`quote.rs` implements `Shell::{Posix, PowerShell, Cmd}` with table-driven tests covering: spaces, single/double quotes, `$`, backtick, `%`, `^`, `&`, `;`, unicode, empty string, trailing backslash. Prefer a vetted crate for POSIX (e.g. `shlex`) plus hand-written PowerShell/cmd with tests. Because §9.2 already forbids most metacharacters, quoting is defence in depth, not the primary control.

---

## 10. Security & privacy

- No telemetry. No network I/O by the app itself.
- Never run `org display` (it can emit access tokens). `org list` hides secrets in 2.150.6 (F8), but the app must still scrub any field named like `accessToken`, `password`, `Password`, `refreshToken`, `sfdxAuthUrl`, `clientSecret`, `privateKey` from every result, whatever its value (F25, F34). The runner removes `SF_TEMP_SHOW_SECRETS` from the child environment (§7.3).
- Installation keys: masked input, memory only, excluded from History/logs/diagnostics.
- Logs (`tracing`): argv of runnable commands is safe (validated, secret-free) and may be logged; stdout is **not** logged by default.
- **Copy diagnostics** redacts usernames, org IDs, instance URLs by default (checkbox to include).
- Dependency hygiene: `cargo deny` + `cargo audit` in CI.
- README disclaimer: "Not affiliated with or endorsed by Salesforce."

---

## 11. Testing

1. **Unit:** validators, `argv()` for every variant (golden tests), quoting tables, alias join (15/18-char), classifier (§5.3), "latest released" computation, lenient parsers on fixtures.
2. **Fake `sf`:** `tests/fake-sf` is a small executable that replays fixtures keyed by argv; set via `GP_ATLAS_SF_BIN`. Integration tests drive the core through it (success, empty, each error class, timeout, huge output).
3. **Contract (CI job `sf-contract`):** Node 22 → `npm i -g @salesforce/cli@2.150.6` → assert `sf version` → run `tools/extract-manifest` → `git diff --exit-code manifest/sf-2.150.6.json`. Also run a test that asserts no runnable variant maps to a blocklisted command. A second, non-blocking job `sf-latest` installs `@salesforce/cli@latest` and runs the same tests with `GP_ATLAS_CONTRACT_MODE=compat` (version gate, bundled plugin, no D4 drift against the baseline manifest).
4. **Fixtures:** `tools/capture-fixtures` runs every runnable command against a real Dev Hub and a real packaging org with `--json`, then sanitizes (usernames, org IDs, instance URLs, record IDs → deterministic fakes). Capture the cases in §4.7 (U1–U7).
5. **Cross-platform CI:** ubuntu, macos, windows (Windows job must exercise spawning `sf.cmd`).
6. **Manual UI checklist** per release: Doctor red/green, Access Matrix with a denied org, 10k-row table scroll, cancel during load, copy in all three shells, alias chip warnings.

---

## 12. Milestones

Each milestone ends with a PR; list the ACs met in the PR description.

| M | Deliverable | Acceptance criteria |
|---|---|---|
| **M0** Bootstrap | Workspace, CI (fmt/clippy/test, `sf-contract`, matrix), `manifest/` + `tools/extract-manifest`, `fake-sf`, README with disclaimer & license. | AC-01, AC-02 |
| **M1** Runner + Doctor | `SfRunner`, envelope parsers, D1–D4, Doctor screen, version gate. | AC-03…AC-08 |
| **M2** Orgs & aliases | `OrgList`/`AliasList`, org selectors, alias display, project chip + `packageAliases` parser. | AC-09…AC-12 |
| **M3** Access Matrix | Probes, classifier, matrix UI, gating, cache. | AC-13…AC-18 |
| **M4** 2GP browse | Package list, version table, filters, "latest released", details, ancestry, deps. | AC-19…AC-26 |
| **M5** 1GP + Installed + Build requests | Screens + commands. | AC-27…AC-30 |
| **M6** Command Builder | Manifest-driven forms, quoting, copy, alias toggles, History. | AC-31…AC-37 |
| **M7** Hardening | Fixtures from real orgs replace heuristics (U1–U9), perf pass, Windows pass, docs. | AC-38…AC-41 |
| **M8** Release | Signed/packaged builds (per-OS), CHANGELOG, v0.1.0. | AC-42 |

---

## 13. Acceptance criteria (testable)

**Foundation**
- **AC-01** `cargo fmt --check`, `clippy -D warnings`, `cargo test` pass on ubuntu/macos/windows.
- **AC-02** `sf-contract` job installs `@salesforce/cli@2.150.6` and the generated manifest equals the committed one.

**CLI contract & Doctor**
- **AC-03** With `sf` 2.150.6 or newer present, Doctor shows D1–D4 green within 8 s of launch on a typical machine.
- **AC-04** With an `sf` older than 2.150.6, every `sf` feature is disabled, a banner shows found vs minimum, and the fix command is copyable. With a newer `sf`, only commands with contract drift are disabled.
- **AC-05** If `@salesforce/plugin-packaging` is not the bundled (`core`) plugin, state is `PackagingPluginOverridden` with an explanation.
- **AC-06** If a manifest flag is missing/changed in `sf commands --json`, only the affected command is disabled and the diff is shown.
- **AC-07** `sf` missing from PATH → clear `CliMissing` state with install instructions; no crash.
- **AC-08** A property test proves `ReadOnlyCommand::argv()` never emits a blocklisted command or a deprecated flag alias.

**Orgs & aliases**
- **AC-09** Org selectors list `devHubs` and other non-scratch orgs from `org list --json` as `alias — username` with type badges.
- **AC-10** With a project chosen, package and version rows show alias chips from `packageAliases`; 15/18-char IDs both match.
- **AC-11** Orphan / unaliased / duplicate alias warnings render correctly on a fixture covering all three.
- **AC-12** GP Atlas never writes to `sfdx-project.json` (test: file hash unchanged after a full UI session on the fake `sf`).

**Access Matrix**
- **AC-13** Each of the five capabilities (§5.2 L2) is probed with exactly the listed command; an empty result is `Allowed`.
- **AC-14** `NamedOrgNotFoundError` → `Unreachable(NotAuthenticated)` with the login hint.
- **AC-15** `INVALID_TYPE … Package …` / "Packaging is not enabled" → `Denied(PackagingNotEnabled)`.
- **AC-16** Timeouts kill the child process and show `Unknown(Timeout)`; no zombie `sf` processes remain.
- **AC-17** Results are cached 15 min, invalidated on org-list change; **Re-test** bypasses the cache.
- **AC-18** Screens requiring a capability show state + reason + **Try anyway** when not `Allowed`.

**2GP browse**
- **AC-19** `package list` rows render; deprecated packages are absent (consistent with F11); `Alias` column comes from the project, not the CLI.
- **AC-20** Version table renders 10,000 rows with smooth scrolling (virtualized) and parses off the UI thread.
- **AC-21** Each filter maps to exactly one documented flag (released, branch, created/modified days, conversions, concise/verbose, order-by).
- **AC-22** `--order-by` accepts only allow-listed fields; invalid input is rejected inline and never reaches argv.
- **AC-23** Branch input containing quotes/spaces is rejected inline (F14).
- **AC-24** **Latest released per package** equals the manual max over `IsReleased` rows for a fixture.
- **AC-25** Ancestry is enabled only for 2GP managed packages; dependencies only for unlocked/2GP managed; disabled with a tooltip otherwise.
- **AC-26** **Copy 04t** and **Copy install link** work; the link equals the CLI's `InstallUrl` for production.

**1GP / Installed / Build requests**
- **AC-27** `package1 version list` (with and without `--package-id`) renders; omitted id = all packages in the org.
- **AC-28** `package installed list` renders `VersionSettings` correctly (`namespace`/`packageId`/blank).
- **AC-29** `create list` supports status + days filters; row → `create report`.
- **AC-30** All three screens honour loading/empty/denied/error/stale states.

**Command Builder**
- **AC-31** Forms are generated from the manifest; no flag is hardcoded in the UI layer.
- **AC-32** Copy output is correct and runnable in bash, zsh, PowerShell and cmd for values with spaces, quotes and `$` (table-driven tests + manual check on each OS).
- **AC-33** Org alias ↔ username and package ID ↔ alias toggles change the preview; alias mode shows the project-directory note and optional `cd` prefix.
- **AC-34** `--json` defaults off for copy and on for app-run.
- **AC-35** Installation keys are masked, excluded from History/logs/diagnostics, and previewed as a placeholder by default.
- **AC-36** History records the last 200 app-run commands with exit code and duration; contains no secrets.
- **AC-37** (v1.1) Mutating templates are copy-only, badged, and the app has no code path that executes them.

**Hardening / release**
- **AC-38** U1–U9 are resolved with real fixtures; heuristic classifier rules replaced by exact matches or documented as unresolved.
- **AC-39** No telemetry; no network I/O by the app (verified by running under a network-blocking sandbox with the fake `sf`).
- **AC-40** Windows `sf.cmd` spawning works and rejects any argv value that fails §9.2.
- **AC-41** README documents install, the exact CLI requirement, the read-only guarantee, and the Salesforce non-affiliation notice.
- **AC-42** Tagged release `v0.1.0` with per-OS artifacts and CHANGELOG.

---

## 14. Dependencies (suggested, not mandatory)

`eframe`/`egui` + `egui_extras` (virtualized tables) · `tokio` (process, time, sync) · `serde`/`serde_json` · `thiserror` · `anyhow` (binary only) · `tracing` + `tracing-subscriber` · `which` · `directories` · `shlex` · `semver` · `regex` (or hand-rolled validators) · `proptest` (argv property tests) · `insta` (snapshot tests) · `cargo-deny`, `cargo-audit` (CI). Pin the Rust toolchain via `rust-toolchain.toml` and set `rust-version` in `Cargo.toml` to what CI uses.

---

## 15. Appendix

### A. Reproduce the verification (what the spec was built from)

```bash
mkdir /tmp/sf2150 && cd /tmp/sf2150 && npm init -y
npm install @salesforce/cli@2.150.6
export PATH="$PWD/node_modules/.bin:$PATH"
sf version --json
sf plugins --json                      # bare array; find @salesforce/plugin-packaging 3.0.6 (core)
sf commands --json > commands.json     # 273 commands; source of manifest/sf-2.150.6.json
sf package version list --help
sf package1 version list --help
sf package list -v nobody@example.com --json   # observe error envelope (exit 2)
```

Plugin source for verification lives under `node_modules/@salesforce/cli/node_modules/@salesforce/{plugin-packaging,packaging}/lib` (e.g. `commands/package/version/list.js`, `package/packageVersionList.js`, `utils/packageUtils.js`).

### B. Fixture capture checklist (for §4.7)

- [ ] `org list --json` with ≥1 Dev Hub, ≥1 sandbox, ≥1 scratch org (U1)
- [ ] `package version report --json` with/without `--verbose` (U2)
- [ ] `package version create list --json`, `create report --json` (U3)
- [ ] `displayancestry --json` with/without `--dot-code`; `displaydependencies --json` (U4)
- [ ] Errors: revoked/expired token, restricted profile, API disabled, offline (U5)
- [ ] `plugins --json` after installing a throwaway user plugin (U6)
- [ ] `package1 version list --json` in a packaging org, a non-packaging org, a subscriber org (U7)
- [ ] Sandbox install link opened in a sandbox (U8)
- [ ] `sf package version list` run from a nested project subfolder (U9)

### C. Naming/branding

Name: **GP Atlas** (repo `gp-atlas`). Tagline: *Browse 1GP and 2GP package versions and build `sf` commands.* Always describe it as a companion to the Salesforce CLI, never as a replacement.

---

## 16. Kickoff message (paste to the coding agent)

> Read `SPEC.md` completely. Confirm back (in ≤10 bullets) the hard rules in §1 and the milestone order in §12. Then execute **M0**: create the Cargo workspace, CI (including the `sf-contract` job pinned to `@salesforce/cli@2.150.6`), `tools/extract-manifest`, the `fake-sf` helper, and the README with the Salesforce non-affiliation notice. Do not write UI code yet. Open a PR listing the ACs satisfied (AC-01, AC-02). If anything in §4 contradicts what you observe from the real CLI, stop and report it instead of working around it.
