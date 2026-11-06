# Roadmap

What comes after the first release, roughly in order. Design notes for the
larger items live in [`docs/ideas/`](docs/ideas/). Suggestions are welcome as
[issues](https://github.com/holapancho/gp-atlas/issues).

## Next

| Item | Notes |
|---|---|
| Installation key for key-protected versions | Masked input, kept in memory for one call, never in History or logs (SPEC §9.1, §10); unlocks `--installation-key` in dependencies and the Command Builder. |
| Hardening | Randomized test that no blocklisted command or flag can ever be built; a check that the app makes no network calls of its own; a Windows `sf.cmd` pass (SPEC M7). |
| Signed releases | macOS notarization and Windows code signing, so no quarantine or SmartScreen steps. |

## Later

| Item | Notes |
|---|---|
| "Who has my package" view | Subscriber orgs per package and version, "orgs below version X", an org file ready for push upgrades. Read-only, via fixed `PackageSubscriber` queries. [Idea](docs/ideas/WRITE_ACTIONS_AND_PUSH_UPGRADES.md#4-push-upgrades--2gp-cli) |
| Push upgrade history | `sf package push-upgrade list` / `report` and per-org errors. Read-only. Same idea doc. |
| Copy-only templates for state-changing commands | Create versions (promotable or not), promote, delete with warnings, schedule / abort push upgrades, 1GP customized push upgrades in Apex. GP Atlas builds and badges them; you run them. [Idea](docs/ideas/WRITE_ACTIONS_AND_PUSH_UPGRADES.md) |

## Waiting on real `sf` outputs

These need captures from real orgs ([how to help](docs/fixture-capture.md)):
exact error names for "not a Dev Hub", expired session, insufficient access,
API disabled and network failure; `package1 version list` in a subscriber
org; the sandbox install link; a successful dependency graph; a version still
being built; the installation-key error.

## Not planned

- A desktop (windowed) app: GP Atlas is a terminal app.
- Running state-changing commands from GP Atlas. If this ever changes, it would
  be an explicit, separate mode (see the write-actions idea, §1).
