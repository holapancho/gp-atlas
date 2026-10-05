# Fixtures for `sf` 2.150.6

Real outputs captured from `@salesforce/cli@2.150.6`. `index.json` maps argv to
fixture files for `tests/fake-sf`.

| File | Captured with | Exit |
|---|---|---|
| `version.json` | `sf version --json` | 0 |
| `plugins.json` | `sf plugins --json` (**trimmed**, see below) | 0 |
| `org-list.empty.json` | `sf org list --json` (no authenticated orgs) | 0 |
| `org-list.real.json` | `sf org list --json` on a developer machine with 48 orgs (3 Dev Hubs, 4 sandboxes, 34 other, 7 scratch); captured by the project owner on darwin-arm64, Node 24.19.0, 2026-10 | 0 |
| `alias-list.empty.json` | `sf alias list --json` (no aliases) | 0 |
| `org-list-skip.derived.json` | **Derived**, not captured: `org-list.real.json` with every `connectedStatus` removed, which is what `--skip-connection-status` omits (F26). Served for `org list --skip-connection-status --json`. Replace with a real capture when available. | 0 |
| `package-list.real.json` | `sf package list --target-dev-hub <hub> --json` against the owner's default Dev Hub (11 packages: 10 managed, 1 unlocked). The `--verbose` capture was byte-identical, so `index.json` serves this file for both. Owner-sanitized; the hub is served under its sanitized alias `fake0009` from `org-list.real.json`. | 0 |
| `package-list.named-org-not-found.json` | `sf package list -v nobody@example.com --json` | 2 |
| `package1-version-list.named-org-not-found.json` | `sf package1 version list -o nobody@example.com --json` | 2 |

Capture environment: 2026-10-01, linux-x64, Node 22.22.0, clean
`npm install @salesforce/cli@2.150.6`, no authenticated orgs.

Sanitization:

- `org-list.real.json`: usernames, org IDs, instance/login URLs, names, aliases, namespaces and client IDs were replaced with fakes by the project owner before upload. Additionally, the 7 `ScratchOrgInfo` record IDs in `attributes.url` were replaced with `2SR000000000000001`…, and every `accessToken` was set to the CLI's own constant redaction text (`[REDACTED] Use 'sf org auth show-access-token' to view`, plugin-org 6.0.11), which is what the CLI prints when `SF_TEMP_SHOW_SECRETS` is unset.

- Local install paths in stack traces were replaced with `/opt/sf`.
- `plugins.json`: the real output is about 3.4 MB because each entry embeds
  its full oclif manifest. Only `name`, `version` and `type` are kept per
  entry, in the original order. All 39 entries are present.
- The two error fixtures were captured with the short flags `-v` / `-o`.
  `index.json` serves them for the canonical long form (`--target-dev-hub` /
  `--target-org`), the only form GP Atlas emits. The output does not depend
  on the spelling.

The remaining real-org captures are listed in `docs/FIXTURE_CAPTURE.md`.
