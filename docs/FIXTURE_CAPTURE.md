# GP Atlas — Fixture capture checklist

The `sf` outputs that **cannot** be learned from the CLI source, schemas or plugin tests
and must come from real orgs (SPEC §4.7: U1 confirmation, U5, U7, U8). Everything else
was resolved from the installed `sf` 2.150.6 and its plugins — see SPEC §4.3 F24–F35.

**Safe to run:** every command is read-only. None changes an org.
**No secrets:** do **not** set `SF_TEMP_SHOW_SECRETS`, and never send `sf org display` output.
**Send failures too:** an error output is as valuable as a success.

---

## Setup

```bash
sf version --json        # must print "cliVersion": "@salesforce/cli/2.150.6"
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
