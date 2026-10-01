# GP Atlas — Fixture capture checklist

Every `sf` output GP Atlas needs as a real fixture (SPEC §4.7, §11.4, Appendix B).
Run these on your machine, zip the folder, and send it back. Captures are sanitized
(usernames, org IDs, instance URLs, record IDs → fakes) before anything is committed.

**Safe to run:** every command here is read-only. None changes an org.
**No secrets:** `sf` 2.150.6 hides tokens. Do **not** set `SF_TEMP_SHOW_SECRETS`, and never send `sf org display` output.
**Send failures too:** an error output is as valuable as a success.

---

## 0. Setup

```bash
sf version --json        # must print "cliVersion": "@salesforce/cli/2.150.6"
mkdir gp-atlas-fixtures && cd gp-atlas-fixtures
```

Define the `cap` helper. It saves stdout, stderr and the exit code of each command.

**bash / zsh**

```bash
cap() { n=$1; shift; sf "$@" --json > "$n.json" 2> "$n.stderr"; echo $? > "$n.exit"; }
```

**PowerShell**

```powershell
function cap($n) { sf @args --json 2> "$n.stderr" | Out-File -Encoding utf8 "$n.json"; $LASTEXITCODE | Out-File -Encoding utf8 "$n.exit" }
```

Set your org aliases. All the commands below work unchanged in both shells.

```bash
HUB=my-devhub-alias       # PowerShell: $HUB = "my-devhub-alias"
PKGORG=my-1gp-org         # PowerShell: $PKGORG = "my-1gp-org"
SUB=my-subscriber-org     # PowerShell: $SUB = "my-subscriber-org"
```

| Variable | Org | Needed for |
|---|---|---|
| `HUB` | Dev Hub with ≥1 **managed** 2GP and ≥1 **unlocked** package, several versions (some released, some not), a branch, ancestry | §2 |
| `PKGORG` | 1GP packaging org (Developer Edition with namespace and an uploaded managed package) | §3 |
| `SUB` | Any org with packages installed, **not** a Dev Hub | §3, §4, §5 |

Skip any section whose org you don't have.

---

## 1. Inventory — no IDs needed

| ✔ | File | Command | Resolves |
|---|---|---|---|
| ☐ | `01-org-list` | `cap 01-org-list org list` | U1 org fields, AC-09 |
| ☐ | `02-org-list-skip` | `cap 02-org-list-skip org list --skip-connection-status` | U1, I1 fast paint |
| ☐ | `03-alias-list` | `cap 03-alias-list alias list` | I2 |
| ☐ | `04-config-get` | `cap 04-config-get config get target-dev-hub target-org` | I3 |

Best if your org list includes at least 1 Dev Hub, 1 sandbox and 1 scratch org.

---

## 2. Dev Hub (2GP)

| ✔ | File | Command | Resolves |
|---|---|---|---|
| ☐ | `10-package-list` | `cap 10-package-list package list --target-dev-hub "$HUB"` | AC-19, probe Pkg2.ListPackages |
| ☐ | `11-package-list-verbose` | `cap 11-package-list-verbose package list --target-dev-hub "$HUB" --verbose` | AC-19 |
| ☐ | `12-version-list` | `cap 12-version-list package version list --target-dev-hub "$HUB"` | AC-20, AC-24, AC-26 |
| ☐ | `13-version-list-verbose` | `cap 13-version-list-verbose package version list --target-dev-hub "$HUB" --verbose` | code coverage column |
| ☐ | `14-version-list-probe` | `cap 14-version-list-probe package version list --target-dev-hub "$HUB" --concise --created-last-days 0` | probe Pkg2.ListVersions, AC-13 |
| ☐ | `15-version-list-released` | `cap 15-version-list-released package version list --target-dev-hub "$HUB" --released` | AC-21 |
| ☐ | `16-version-list-orderby-all` | `cap 16-version-list-orderby-all package version list --target-dev-hub "$HUB" --order-by "CreatedDate,LastModifiedDate,MajorVersion,MinorVersion,PatchVersion,BuildNumber,Package2Id,Branch,Name,IsReleased"` | AC-22 order-by allow-list |
| ☐ | `17-version-list-orderby-desc` | `cap 17-version-list-orderby-desc package version list --target-dev-hub "$HUB" --order-by "CreatedDate DESC"` | AC-22 ASC/DESC |
| ☐ | `18-create-list-probe` | `cap 18-create-list-probe package version create list --target-dev-hub "$HUB" --created-last-days 0` | U3, probe Pkg2.CreateRequests |
| ☐ | `19-create-list` | `cap 19-create-list package version create list --target-dev-hub "$HUB"` | U3, AC-29 |

Now pick IDs from the outputs above. Skip lines for IDs you don't have.

```bash
M0HO=0Ho...      # a MANAGED package          (from 10)
MV04T=04t...     # a version of it            (from 12)
U0HO=0Ho...      # an UNLOCKED package        (from 10)
UV04T=04t...     # a version of it            (from 12)
REQ08C=08c...    # a version-create request   (from 19)
```

PowerShell: `$M0HO = "0Ho..."`, and so on.

| ✔ | File | Command | Resolves |
|---|---|---|---|
| ☐ | `20-report` | `cap 20-report package version report --target-dev-hub "$HUB" --package "$MV04T"` | U2 |
| ☐ | `21-report-verbose` | `cap 21-report-verbose package version report --target-dev-hub "$HUB" --package "$MV04T" --verbose` | U2 |
| ☐ | `22-ancestry-pkg` | `cap 22-ancestry-pkg package version displayancestry --target-dev-hub "$HUB" --package "$M0HO"` | U4 |
| ☐ | `23-ancestry-pkg-dot` | `cap 23-ancestry-pkg-dot package version displayancestry --target-dev-hub "$HUB" --package "$M0HO" --dot-code` | U4, F21 |
| ☐ | `24-ancestry-version` | `cap 24-ancestry-version package version displayancestry --target-dev-hub "$HUB" --package "$MV04T"` | U4 |
| ☐ | `25-ancestry-unlocked` | `cap 25-ancestry-unlocked package version displayancestry --target-dev-hub "$HUB" --package "$U0HO"` | AC-25 (expected error) |
| ☐ | `26-deps-unlocked` | `cap 26-deps-unlocked package version displaydependencies --target-dev-hub "$HUB" --package "$UV04T"` | F21, AC-25 |
| ☐ | `27-deps-managed` | `cap 27-deps-managed package version displaydependencies --target-dev-hub "$HUB" --package "$MV04T"` | F21, AC-25 |
| ☐ | `28-create-report` | `cap 28-create-report package version create report --target-dev-hub "$HUB" --package-create-request-id "$REQ08C"` | U3, AC-29 |

---

## 3. 1GP

```bash
P033=033...      # a 1GP package id     (from 30)
P1V04T=04t...    # one of its versions  (from 30)
```

| ✔ | File | Command | Resolves |
|---|---|---|---|
| ☐ | `30-pkg1-list` | `cap 30-pkg1-list package1 version list --target-org "$PKGORG"` | AC-27, probe Pkg1.ListVersions |
| ☐ | `31-pkg1-list-filtered` | `cap 31-pkg1-list-filtered package1 version list --target-org "$PKGORG" --package-id "$P033"` | AC-27 |
| ☐ | `32-pkg1-display` | `cap 32-pkg1-display package1 version display --target-org "$PKGORG" --package-version-id "$P1V04T"` | 1GP Details |
| ☐ | `33-pkg1-list-subscriber` | `cap 33-pkg1-list-subscriber package1 version list --target-org "$SUB"` | U7 subscriber org |
| ☐ | `34-pkg1-list-devhub` | `cap 34-pkg1-list-devhub package1 version list --target-org "$HUB"` | U7 non-packaging org |

---

## 4. Installed packages

| ✔ | File | Command | Resolves |
|---|---|---|---|
| ☐ | `40-installed-sub` | `cap 40-installed-sub package installed list --target-org "$SUB"` | AC-28, probe Pkg2.InstalledList |
| ☐ | `41-installed-hub` | `cap 41-installed-hub package installed list --target-org "$HUB"` | AC-28 |

Ideal: `SUB` has installed packages covering all three `VersionSettings` values (`namespace`, `packageId`, blank).

---

## 5. Error cases

| ✔ | File | How | Resolves |
|---|---|---|---|
| ☐ | `50-pkglist-not-devhub` | `cap 50-pkglist-not-devhub package list --target-dev-hub "$SUB"` | F20, AC-15 |
| ☐ | `51-offline` | Disconnect Wi-Fi/VPN, then `cap 51-offline package list --target-dev-hub "$HUB"` | U5 network |
| ☐ | `52-revoked` *(optional)* | In a **throwaway** org: Setup → Connected Apps OAuth Usage → Salesforce CLI → Revoke, then `cap 52-revoked package list --target-dev-hub <that alias>` | U5 expired session |
| ☐ | `53-restricted` *(optional)* | Log in as a user on a minimal profile, then `cap 53-restricted package version list --target-dev-hub <that alias>` | U5 insufficient access, AC-18 |
| ☐ | `55-api-disabled` *(optional)* | Same, with a user whose profile lacks "API Enabled": `cap 55-api-disabled package list --target-dev-hub <that alias>` | U5 API disabled |

---

## 6. Optional extras

| ✔ | File | How | Resolves |
|---|---|---|---|
| ☐ | `54-nested` | Inside an sfdx project **subfolder** (e.g. `force-app/main`): `cap 54-nested package version list --target-dev-hub "$HUB"`. Note the folder you ran it from. | U9 alias resolution |
| ☐ | `56-in-project` | From the sfdx project **root**: `cap 56-in-project package version list --target-dev-hub "$HUB"`. Also send that project's `sfdx-project.json`. | F13 aliases, AC-10, AC-11 |
| ☐ | `U8` | In a sandbox, open `https://test.salesforce.com/packaging/installPackage.apexp?p0=<04t>` and tell me whether it reaches the install page | U8 sandbox link |
| ☐ | `57-plugins-user` *(optional, changes your CLI)* | `sf plugins install @salesforce/plugin-community@4.0.5` (not bundled; small), then `cap 57-plugins-user plugins`, then `sf plugins uninstall @salesforce/plugin-community` | U6 plugin `type` |

---

## Send back

Zip the whole `gp-atlas-fixtures` folder, including the `.json`, `.stderr` and `.exit` files, and attach it.
If you prefer, find-and-replace your usernames in it first; I sanitize everything anyway.
