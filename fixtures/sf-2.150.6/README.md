# Fixtures for `sf` 2.150.6

Real outputs captured from `@salesforce/cli@2.150.6`. `index.json` maps argv to
fixture files for `tests/fake-sf`.

| File | Captured with | Exit |
|---|---|---|
| `version.json` | `sf version --json` | 0 |
| `plugins.json` | `sf plugins --json` (**trimmed**, see below) | 0 |
| `org-list.empty.json` | `sf org list --json` (no authenticated orgs) | 0 |
| `alias-list.empty.json` | `sf alias list --json` (no aliases) | 0 |
| `package-list.named-org-not-found.json` | `sf package list -v nobody@example.com --json` | 2 |
| `package1-version-list.named-org-not-found.json` | `sf package1 version list -o nobody@example.com --json` | 2 |

Capture environment: 2026-10-01, linux-x64, Node 22.22.0, clean
`npm install @salesforce/cli@2.150.6`, no authenticated orgs.

Sanitization:

- Local install paths in stack traces were replaced with `/opt/sf`.
- `plugins.json`: the real output is about 3.4 MB because each entry embeds
  its full oclif manifest. Only `name`, `version` and `type` are kept per
  entry, in the original order. All 39 entries are present.
- The two error fixtures were captured with the short flags `-v` / `-o`.
  `index.json` serves them for the canonical long form (`--target-dev-hub` /
  `--target-org`), the only form GP Atlas emits. The output does not depend
  on the spelling.

Fixtures that need real orgs (Dev Hub, packaging org, sandbox; SPEC §4.7
U1–U9 and Appendix B) are still to be captured with `tools/capture-fixtures`
(milestone M7).
