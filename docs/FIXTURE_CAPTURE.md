# GP Atlas — Fixture capture checklist

The `sf` outputs that **cannot** be learned from the CLI source, schemas or plugin tests
and must come from real orgs (SPEC §4.7: U5, U7, U8, and the ancestry/dependency
unknowns U10–U14 in `docs/ideas/ANCESTRY_AND_DEPENDENCIES.md`). Everything else
was resolved from the installed `sf` 2.150.6 and its plugins — see SPEC §4.3 F24–F43.

Captures can also be pasted as anonymized Markdown (command, JSON, notes), as
was done for ancestry, dependencies and 1GP.

**Safe to run:** every command is read-only. None changes an org.
**No secrets:** do **not** set `SF_TEMP_SHOW_SECRETS`, and never send `sf org display` output.
**Send failures too:** an error output is as valuable as a success.

---

## Setup

```bash
sf version --json        # "cliVersion" must be @salesforce/cli/2.150.6 or newer; say which one
mkdir gp-atlas-fixtures && cd gp-atlas-fixtures
```

`cap` helper — saves stdout, stderr and the exit code of each command.

**bash / zsh**

```bash
cap() { n=$1; shift; sf "$@" --json > "$n.json" 2> "$n.stderr"; echo $? > "$n.exit"; }
HUB=my-devhub-alias      # your Dev Hub
SUB=my-subscriber-org    # an ordinary org: NOT a Dev Hub, NOT a 1GP packaging org
```

**PowerShell**

```powershell
function cap($n) { sf @args --json 2> "$n.stderr" | Out-File -Encoding utf8 "$n.json"; $LASTEXITCODE | Out-File -Encoding utf8 "$n.exit" }
$HUB = "my-devhub-alias"
$SUB = "my-subscriber-org"
```

---

## The 8 captures (1 done, 7 to go)

U7 (#2) is the next most useful: the same command already captured on your 1GP
packaging org, now on an org where the package is only installed.

Easiest first: 2–4 need only your Dev Hub and one ordinary org.

| ✔ | # | Resolves | How |
|---|---|---|---|
| ✅ | 1 | U1 `org list` fields | Received — `fixtures/sf-2.150.6/org-list.real.json` |
| ☐ | 2 | U7 1GP in a subscriber org | `cap 33-pkg1-list-subscriber package1 version list --target-org "$SUB"` |
| ☐ | 3 | U5 org is not a Dev Hub | `cap 50-pkglist-not-devhub package list --target-dev-hub "$SUB"` |
| ☐ | 4 | U5 offline | Turn Wi-Fi/VPN off, then `cap 51-offline package list --target-dev-hub "$HUB"` |
| ☐ | 5 | U5 expired session | In a **throwaway** org: Setup → Connected Apps OAuth Usage → Salesforce CLI → Revoke, then `cap 52-revoked package list --target-dev-hub <that alias>` |
| ☐ | 6 | U5 insufficient access | Log in as a user on a minimal profile, then `cap 53-restricted package version list --target-dev-hub <that alias>` |
| ☐ | 7 | U5 API disabled | Log in as a user whose profile lacks "API Enabled", then `cap 55-api-disabled package list --target-dev-hub <that alias>` |
| ☐ | 8 | U8 sandbox install link | No command: in a sandbox, open `https://test.salesforce.com/packaging/installPackage.apexp?p0=<04t>` and report whether the install page loads |

---

## Received so far (outside the table)

| Unknown | File | Note |
|---|---|---|
| U5 org gone (HTTP 420) | `package-installed-list.http-420.json` | SPEC F38 |
| U12 ancestry of a whole package | `package-version-displayancestry.dot*.json` | SPEC F41: synthetic `nodeunknown` root |
| U11 dependency error | `package-version-displaydependencies.invalid-dependency-graph.json` | SPEC F42/F43: misleading message |
| 1GP list in the packaging org | `package1-version-list.real.json`, `…did-you-mean.json` | sample rows, `actions` suggestion |

Still wanted for ancestry/dependencies:

| ☐ | Unknown | Command |
|---|---|---|
| ☐ | U10 successful dependency graph | `cap 60-deps package version displaydependencies --target-dev-hub "$HUB" --package <04t of a version that depends only on your own packages> --edge-direction root-last` (and the same with `--verbose`) |
| ☐ | U13 version still building | the same with `--package <08c>` during a build |
| ☐ | U14 installation key | the same on a key-protected version without the key (never send the key) |

---

## Nice to have (not blocking)

Realistic sample data for tests. Shapes are already known (SPEC F27–F30).

| ✔ | File | Command |
|---|---|---|
| ☐ | `12-version-list` | `cap 12-version-list package version list --target-dev-hub "$HUB"` |
| ☐ | `13-version-list-verbose` | `cap 13-version-list-verbose package version list --target-dev-hub "$HUB" --verbose` |
| ☐ | `40-installed-sub` | `cap 40-installed-sub package installed list --target-org "$SUB"` |

---

## Send back

Zip the whole `gp-atlas-fixtures` folder (`.json`, `.stderr`, `.exit`) and attach it.
Usernames, org IDs, instance URLs and record IDs are replaced with fakes before anything is committed.
