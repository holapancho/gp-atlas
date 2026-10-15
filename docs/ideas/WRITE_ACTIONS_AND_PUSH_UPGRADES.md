# Idea: write actions (create, promote, delete) and push upgrades

Status: **idea, not built** (recorded 2026-10-15 at the owner's request).
Nothing here is allowed today: GP Atlas is read-only (SPEC §1 rule 1) and the
runner only accepts the closed `ReadOnlyCommand` set. Building any part that
*runs* a state-changing command needs an explicit owner decision to change
that rule (see §1).

Owner's wishes, in their words: create promotable and non-promotable package
versions; maybe build delete commands, with several warnings; push upgrades
for 2GP through the CLI and for 1GP through Apex run from the CLI.

Sources: the Salesforce docs site cannot be reached from the build
environment, so the owner pasted the two pages (2GP "Schedule a Push Upgrade
Using CLI" and "Enable a Package Subscriber to Restrict Push Upgrades").
Command flags were also checked in `plugin-packaging` 3.0.6 source and
messages (the plugin bundled in sf 2.150.6). Only §5.2 (scheduling a 1GP push)
is still from general knowledge and must be verified.

---

## 1. The rule question (decide first)

Two ways to offer these actions:

| Mode | What GP Atlas does | Fits today's SPEC? |
|---|---|---|
| **A. Copy-only templates** | Builds the exact command in the Command Builder, badged "⚠ changes state — GP Atlas will not run this"; you paste it in your terminal. | ✅ Yes: SPEC §9.1 already plans v1.1 copy-only mutating templates. |
| **B. Run from GP Atlas ("write mode")** | GP Atlas runs the command itself after confirmations. | ❌ Needs rule 1 changed. |

Recommendation: ship **A** first (no rule change, no risk), and only consider
**B** for specific actions after A has been used for a while. If B is ever
chosen, keep it out of the read-only core: a separate, clearly named
`WriteCommand` enum, off by default (`--allow-writes` or a separate binary),
never reachable from the read-only runner, with the confirmations in §6.

## 2. Create package versions (2GP)

`sf package version create` (blocklisted today: final segment `create`).

- **Promotable** (can be released later with `promote`): validation runs, and
  `--code-coverage` computes and stores coverage by running the packaged
  Apex tests (promotion needs enough coverage; check the current threshold). Optional `--async-validation` returns earlier and
  validates in the background.
- **Not promotable** (fast builds for testing): `--skip-validation`. Such a
  version can never be promoted ("you can't promote unvalidated package
  versions", CLI message). The template must say so in the preview.
- Useful extras for the form (verified in 3.0.6 `version/create.ts`):
  `--installation-key` / `--installation-key-bypass` (one of them is
  required), `--version-number`, `--version-name`, `--version-description`,
  `--skip-ancestor-check`, `--definition-file`; `--branch`, `--tag`,
  `--wait` to be confirmed from the manifest, and reminding the user about `calculateTransitiveDependencies`
  in `sfdx-project.json` (so `d` shows the full graph later, SPEC F42/F43).
- Read-only follow-up already in GP Atlas: `package version create list` /
  `report` (build status).
- Note: `create` updates `sfdx-project.json` (`packageAliases`, F40). GP
  Atlas itself never edits that file; the CLI does.

`sf package version promote --package <04t>`: releases a version. It is
permanent; show the version, the package and "this cannot be undone".

## 3. Delete (packages and versions)

`sf package version delete --package <04t>` and `sf package delete --package <0Ho>`
(both blocklisted: final segment `delete`).

Warnings to show before the command is even copyable:

1. What will be deleted: package name, version number, 04t/0Ho, released or
   not, how many versions (for a package).
2. The CLI's own rule (plugin-packaging 3.0.6 `package_version_delete.md`):
   "In second-generation managed packaging, only beta package versions can
   be deleted"; it links Salesforce's package-deletion considerations, which
   the warning should link too. `package version delete --undelete` restores
   a deleted version: mention it.
3. Typed confirmation: the user types the version number (or package name)
   to unlock **Copy**; never a single keypress.
4. One item at a time: no "delete all", no multi-select delete.
5. A red badge, and the target Dev Hub shown in full (alias and username).

## 4. Push upgrades — 2GP (CLI)

Sources: Salesforce's "Schedule a Push Upgrade Using CLI" (pkg2-dev guide,
pasted by the owner 2026-10-15) and `plugin-packaging` 3.0.6 source/messages.

**Requirements:** unlocked and second-generation managed packages only.
Managed 2GP must have passed the AppExchange (now "AgentExchange") security
review and the partner must log a case with Partner Support to enable push
upgrades; unlocked packages have them by default. The user needs the
"Create and Update Second-Generation Packages" permission. Everything runs
against the **Dev Hub that owns the package**.

**Workflow from the guide:**

1. **Pick the orgs.** No push-upgrade command for this; the guide uses
   `sf data query` on the Dev Hub:
   - all orgs with the package (033):
     `SELECT OrgKey, OrgName, OrgType, InstanceName, MetadataPackageId, MetadataPackageVersionId FROM PackageSubscriber WHERE MetadataPackageId = '033…'`
   - orgs on one version (04t): `… FROM PackageSubscriber WHERE MetadataPackageVersionId = '04t…'` (`--result-format csv`)
   - orgs below a version (e.g. < 2.7), two steps: Tooling
     `SELECT SubscriberPackageVersionId FROM Package2Version WHERE Package2Id = '0Ho…' AND (MajorVersion < 2 OR (MajorVersion = 2 AND MinorVersion < 7))`
     (`--use-tooling-api`), then `SELECT OrgKey FROM PackageSubscriber WHERE MetadataPackageVersionId IN (…)`.
   - The 033 and 0Ho come from `package list --verbose`; 04ts from
     `package version list`. For `--org-file`, the CSV must contain **one
     org ID per line and no header** (remove the first line of the query CSV).
2. **Schedule:** `sf package push-upgrade schedule --package <04t>
   (--org-list 00D…,00D… | --org-file orgs.csv) [--start-time "YYYY-MM-DDTHH:MM:SS"]
   [--migrate-to-2gp] --target-dev-hub <hub>`. Start time is **UTC**; without
   it the upgrade starts as soon as resources are available (possibly hours
   later); schedule off-peak. The 04t must be an active, non-beta version and
   every org must have a lower version installed.
3. **Track:** `sf package push-upgrade list --package <033> [--scheduled-last-days N]
   [--status Created|Cancelled|Pending|In Progress|Failed|Succeeded]
   [--show-push-migrations-only] --target-dev-hub <hub>` shows request ID
   (`0DV…`), version ID and number, status, scheduled start, orgs scheduled /
   succeeded / failed, created date. `sf package push-upgrade report
   --push-request-id <0DV> --target-dev-hub <hub>` adds error details.
   `Failed` means it failed for one or more orgs.
4. **Abort:** `sf package push-upgrade abort --push-request-id <0DV>`, only for
   `Created` or `Pending` requests.
5. **Errors per org:** `sf data query` on the Dev Hub:
   `SELECT Id, PackagePushJobId, PackagePushJob.SubscriberOrganizationKey, ErrorDetails, ErrorMessage, ErrorSeverity, ErrorTitle, ErrorType FROM PackagePushError WHERE PackagePushJob.PackagePushRequestId = '0DV…'`

| Command | Changes state? |
|---|---|
| `package push-upgrade schedule`, `abort` | **yes** (blocklisted: `schedule`, `abort`) |
| `package push-upgrade list`, `report` | no |
| the `data query` SOQL above | no (but `data query` takes arbitrary SOQL, see below) |

**What GP Atlas could do without breaking the read-only rule** (best value
first):

- **"Who has my package" view:** subscriber orgs per package/version from
  `PackageSubscriber` (org ID, name, type, instance, installed 04t), with a
  version histogram and "orgs below version X". Needs `data query` added to
  `ReadOnlyCommand` as **fixed queries only** (GP Atlas builds the SOQL from
  validated IDs/numbers; never user-written SOQL), plus fixtures.
- **Push history view:** `push-upgrade list` / `report` per package, and
  per-org errors from `PackagePushError`.
- **Org file for scheduling:** copy or save the selected subscribers' `00D`
  IDs as a header-less CSV, ready for `--org-file` (saving a file the user
  asks for is not an org change).
- **Copy-only schedule/abort templates** (mode A) with the warnings of §6.

## 5. Push upgrades — 1GP, and "customized push upgrades" (Apex)

### 5.1 Customized push upgrades (let a customer block pushes)

Source: Salesforce's "Enable a Package Subscriber to Restrict Push Upgrades"
(pasted by the owner 2026-10-15). Applies to 1GP (run in the **1GP packaging
org**) and managed 2GP (run in the **Dev Hub**), as a system administrator, in
Developer Console → Execute Anonymous. Apex class
`PushUpgradeCustomizationRepository`:

| Call | Effect |
|---|---|
| `create('<033>', '<00D>', true)` | Lets that customer org block push upgrades of that package; returns the record ID (prefix `12C`), e.g. logged as `pucId1 =12C…` |
| `create('<033>', '<00D>', true, 90)` | Same, expiring after 90 days |
| `setExpirationDaysForIndex('<033>', '<00D>', 100)` | Changes the expiry days for that package/org |
| `setExpirationDaysForId('<12C…>', 120)` | Changes the expiry days for one record |

One call per production org; sandboxes inherit their production org's
setting. The **customer** then blocks pushes in Setup → Installed Packages →
Block Push Upgrades (and can Allow Push Upgrades again); while blocked,
upgrades are manual only. Not possible on a **patch** version (they must
upgrade to a non-patch version first). Expiry days cannot be **decreased**
once the customer has blocked.

From the CLI the same Apex can run with `sf apex run --target-org <packaging
org or Dev Hub> --file puc.apex` (anonymous Apex, like Execute Anonymous; to
verify). GP Atlas could generate that file from validated `033` / `00D` /
days values and show it, copy-only.

### 5.2 Scheduling a 1GP push upgrade

Not covered by the two pages above. 1GP has no `package1 push-upgrade`
command; pushes are scheduled through API objects in the packaging org,
which anonymous Apex or `sf data` commands can drive. From general knowledge
(**unverified — needs the 1GP push-upgrade API page**): insert a
`PackagePushRequest` (`PackageVersionId` = target 04t, `ScheduledStartTime`),
insert one `PackagePushJob` per org (`PackagePushRequestId`,
`SubscriberOrganizationKey`), then set the request `Status` to `Pending`;
abort by setting it to `Canceled`; follow `PackagePushRequest.Status`,
`PackagePushJob.Status` and `PackagePushError`. Subscribers come from
`PackageSubscriber` (as in §4) and versions from `MetadataPackageVersion`.

## 6. Safety design for any write action

- Preview of the exact command, the target org (alias **and** username) and
  whether it is a production org (Dev Hubs are production).
- Confirmation proportional to impact: typed confirmation for promote,
  delete, schedule, abort; never a default "yes".
- Installation keys: masked, never stored, never in History (SPEC §9.1, §10).
- If write mode (B) is ever built: a local, secret-free audit log of what was
  run and when; no batch operations; the read-only binary stays read-only.

## 7. Unknowns to capture first

| ID | Unknown |
|---|---|
| W1 | `push-upgrade list` / `report` JSON shapes (real capture on a package with push history) |
| W2 | Delete rules beyond "only beta versions in 2GP managed" (unlocked packages, whole packages): read the deletion-considerations page |
| W3 | Scheduling a **1GP** push upgrade (§5.2): the API objects/fields and the start step (Salesforce's 1GP push-upgrade API page) |
| W5 | `sf data query` JSON for `PackageSubscriber` and `PackagePushError` on a real Dev Hub / packaging org (for the read-only views) |
| W6 | Whether `sf apex run` output shows the `System.debug` record ID for §5.1 |
| W4 | Whether `push-upgrade` is in `sf commands --json` of 2.150.6 with these flags (manifest/contract) |

## 8. Suggested order

1. Command Builder (read-only commands), now.
2. Read-only views: "who has my package" (`PackageSubscriber`, fixed
   queries) and push history (`push-upgrade list` / `report`,
   `PackagePushError`), after W1/W4/W5.
3. Copy-only templates (mode A): version create (promotable / not), promote,
   push-upgrade schedule/abort, 1GP push Apex, then delete with the §3 warnings.
4. Only after that, and only if the owner changes rule 1: write mode (B).
