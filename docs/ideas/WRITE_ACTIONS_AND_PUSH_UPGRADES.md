# Idea: write actions (create, promote, delete) and push upgrades

Status: **idea, not built** (recorded 2026-10-15 at the owner's request).
Nothing here is allowed today: GP Atlas is read-only (SPEC §1 rule 1) and the
runner only accepts the closed `ReadOnlyCommand` set. Building any part that
*runs* a state-changing command needs an explicit owner decision to change
that rule (see §1).

Owner's wishes, in their words: create promotable and non-promotable package
versions; maybe build delete commands, with several warnings; push upgrades
for 2GP through the CLI and for 1GP through Apex run from the CLI.

Sources: the two pages the owner linked (2GP "Push a package upgrade with
the CLI", 1GP "Push upgrades, customized / Apex") could **not** be fetched
from the build environment (network policy). The 2GP facts below come from
`plugin-packaging` 3.0.6 source and messages (the plugin bundled in sf
2.150.6). The 1GP Apex part is from general Salesforce knowledge and **must be
checked against the owner's link** before anything is built.

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

- **Promotable** (can be released later with `promote`): validation runs and
  code coverage is computed, e.g. `--code-coverage` (managed packages need
  ≥ 75% to promote). Optional `--async-validation` returns earlier and
  validates in the background.
- **Not promotable** (fast builds for testing): `--skip-validation`. Such a
  version can never be promoted. The template must say so in the preview.
- Useful extras for the form: `--installation-key` / `--installation-key-bypass`
  (one of them is required), `--branch`, `--tag`, `--version-number`,
  `--wait`, and reminding the user about `calculateTransitiveDependencies`
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
2. Released versions of managed packages cannot be deleted; installed
   versions break subscribers' upgrade paths. (Verify the exact CLI rules
   before building.)
3. Typed confirmation: the user types the version number (or package name)
   to unlock **Copy**; never a single keypress.
4. One item at a time: no "delete all", no multi-select delete.
5. A red badge, and the target Dev Hub shown in full (alias and username).

## 4. Push upgrades — 2GP (CLI)

From `plugin-packaging` 3.0.6 (`src/commands/package/push-upgrade/*`,
`messages/package_pushupgrade_*.md`):

| Command | Changes state? | Flags |
|---|---|---|
| `sf package push-upgrade schedule` | **yes** (blocklisted: `schedule`) | `-v/--target-dev-hub` (required), `-p/--package <04t>` (required; target version, must be active and non-beta), `-t/--start-time` (UTC, e.g. `"2024-12-06T21:00:00"`; omitted = as soon as possible), `-l/--org-list <00D,…>` or `-f/--org-file <csv>` (one is required; one org ID per row), `--migrate-to-2gp` (push a converted 2GP onto orgs with the 1GP) |
| `sf package push-upgrade abort` | **yes** (blocklisted: `abort`) | `-v`, `-i/--push-request-id` (required). Only `Created` or `Pending` requests can be aborted. |
| `sf package push-upgrade list` | no | `-v`, `-p/--package <033>` (required), `-l/--scheduled-last-days`, `-s/--status` (`Created`, `Cancelled`, `Pending`, `In Progress`, `Failed`, `Succeeded`), `--show-push-migrations-only` |
| `sf package push-upgrade report` | no | `-v`, `-i/--push-request-id` (required) |

Preconditions from the CLI's own help: the user needs "Create and Update
Second-Generation Packages"; for **managed** 2GP, push upgrades require a
passed AppExchange security review and a support case to enable them;
**unlocked** packages have them by default. Every listed org must have a
lower version installed. Salesforce may start the job hours after the start
time; off-peak scheduling is recommended.

**Read-only part that could come before any write mode:** `push-upgrade list`
and `report` are read-only (already in the blocklist's allow tests). They
could join `ReadOnlyCommand` and get a "Push upgrades" view per package
(history, status, errors per org). That needs the commands added to the
manifest and the contract test, like the others.

GP Atlas could also help build the org list: from a subscriber list it cannot
read today (no allow-listed command returns subscribers for 2GP); the user
supplies the CSV. The template would validate each line as a `00D` ID.

## 5. Push upgrades — 1GP (Apex through the CLI)

1GP has no `sf package1 push-upgrade` command. Push upgrades are done with the
packaging org's API objects, which can be driven by anonymous Apex run with
`sf apex run --target-org <packaging org> --file push.apex`.

From general knowledge (**verify against the linked page**):

1. Find the target version: `MetadataPackageVersion` (`Id` = 04t,
   `ReleaseState = 'Released'`), package `MetadataPackage` (033).
2. Find eligible subscribers: `PackageSubscriber` (`OrgKey`,
   `MetadataPackageVersionId`, `InstalledStatus = 'i'`, `OrgType`,
   `OrgStatus`), keeping orgs on a lower version.
3. Insert a `PackagePushRequest` (`PackageVersionId` = target 04t,
   `ScheduledStartTime`).
4. Insert one `PackagePushJob` per org (`PackagePushRequestId`,
   `SubscriberOrganizationKey`).
5. Start it: update the request's `Status` to `Pending`. Abort: set it to
   `Canceled`.
6. Follow it: `PackagePushRequest.Status`, `PackagePushJob.Status`, and
   `PackagePushError` (`ErrorMessage`, `ErrorDetails`, `ErrorSeverity`).

Template idea: GP Atlas fills a reviewed Apex file from parameters (version,
start time, org IDs) and shows `sf apex run --file …` to copy. The Apex text
must be generated from validated values only (04t/033/00D patterns, ISO time),
never from free text, and shown in full before copying.

Read-only parts (monitoring) would be SOQL queries on `PackagePushRequest`,
`PackagePushJob`, `PackagePushError`, `PackageSubscriber`. GP Atlas does not
allow `sf data query` today (it takes arbitrary SOQL). If wanted, add it as a
**fixed** query per view inside `ReadOnlyCommand` (no user-written SOQL),
plus fixtures.

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
| W2 | Exact delete rules for released/installed versions (CLI messages) |
| W3 | The owner's 1GP Apex page: object/field names and the start step |
| W4 | Whether `push-upgrade` is in `sf commands --json` of 2.150.6 with these flags (manifest/contract) |

## 8. Suggested order

1. Command Builder (read-only commands), now.
2. Read-only push-upgrade views (`list`, `report`), after W1/W4.
3. Copy-only templates (mode A): version create (promotable / not), promote,
   push-upgrade schedule/abort, 1GP push Apex, then delete with the §3 warnings.
4. Only after that, and only if the owner changes rule 1: write mode (B).
