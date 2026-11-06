---
title: Home
nav_order: 1
description: "GP Atlas: a clickable terminal app to browse 1GP and 2GP package versions and build sf commands. Read-only."
---

<div class="hero">
  <span class="kicker">For Salesforce package developers</span>
  <h1>Every package version, every org, every <code>sf</code> command — in one terminal app.</h1>
  <p class="lead">GP Atlas runs next to the Salesforce CLI and shows your 1GP and 2GP packages, how
  their versions relate, what each org lets you read, and the exact commands to run next.
  It uses your installed <code>sf</code> and is <strong>read-only</strong>: it never changes an org.</p>
  <div class="hero-actions">
    <a class="btn btn-primary" href="{{ '/install.html' | relative_url }}">Install</a>
    <a class="btn btn-outline" href="{{ '/guide.html' | relative_url }}">User guide</a>
    <a class="btn btn-outline" href="https://github.com/holapancho/gp-atlas">GitHub</a>
  </div>
  <div class="one-liner">curl -fsSL https://raw.githubusercontent.com/holapancho/gp-atlas/main/install.sh | sh</div>
  <p class="fine">macOS · Linux · Windows (<a href="{{ '/install.html' | relative_url }}" style="color:#8ccaff">PowerShell and Cargo installs</a>) · needs <code>sf</code> 2.150.6 or newer · MIT · not affiliated with Salesforce</p>
</div>

<figure class="screen">
  <img src="{{ '/assets/images/packages.png' | relative_url }}" alt="GP Atlas showing the packages of a Dev Hub on the left and the versions of Acme Core on the right, with project aliases">
  <figcaption>Packages and their versions side by side, with your project's aliases. Demo data.</figcaption>
</figure>

## Why GP Atlas

<div class="cards">
  <div class="card"><div class="icon">🧭</div><h3>See the whole picture</h3><p>Packages, versions, released or not, latest per package, install links and full version details, without memorizing flags.</p></div>
  <div class="card"><div class="icon">🌳</div><h3>Ancestry at a glance</h3><p>A package's version tree, with the path from any version back to its root and what was built on it.</p></div>
  <div class="card"><div class="icon">🔗</div><h3>Dependencies in install order</h3><p>What to install first, and whether each dependency is already in the target org, older or missing.</p></div>
  <div class="card"><div class="icon">🛂</div><h3>Know what each org allows</h3><p>The Access Matrix probes every org and explains every "no": not a Dev Hub, expired session, deleted org, with a fix to copy.</p></div>
  <div class="card"><div class="icon">🛠️</div><h3>Commands you can trust</h3><p>Forms built from the CLI's own flag list, validated values, and the exact command quoted for bash/zsh, PowerShell or cmd.</p></div>
  <div class="card"><div class="icon">🔒</div><h3>Read-only by design</h3><p>A fixed list of read-only commands. Nothing that creates, promotes, installs or deletes can run from GP Atlas.</p></div>
</div>

## A closer look

<div class="showcase">
  <div class="text">
    <h3>🌳 Ancestry</h3>
    <p>Press <code>a</code> on a version to see the package's released version tree. The path from that
    version to its root is highlighted, and "Built on it" shows how many versions descend from each one.
    Indentation only grows where the tree forks, so long version lines stay readable.</p>
  </div>
  <img src="{{ '/assets/images/ancestry.png' | relative_url }}" alt="Ancestry tree of Acme Core with the path from 2.3.0.4 to the root highlighted">
</div>

<div class="showcase">
  <div class="text">
    <h3>🔗 Dependencies</h3>
    <p>Press <code>d</code> to get the install order for a version, checked against the selected org:
    <strong>installed</strong>, <strong>older</strong> or <strong>missing</strong>. When the CLI cannot build the
    graph, GP Atlas explains why and shows the direct dependencies instead.</p>
  </div>
  <img src="{{ '/assets/images/dependencies.png' | relative_url }}" alt="Dependencies of Acme Billing in install order, with installed, older and missing states">
</div>

<div class="showcase">
  <div class="text">
    <h3>🛂 Access Matrix</h3>
    <p>One row per org, one column per thing GP Atlas can read. Every result is classified, explained,
    and kept for 15 minutes, so reopening the app is instant.</p>
  </div>
  <img src="{{ '/assets/images/access.png' | relative_url }}" alt="Access Matrix with allowed, denied and unreachable results per org">
</div>

<div class="showcase">
  <div class="text">
    <h3>🛠️ Command Builder</h3>
    <p>Pick a command, fill a form generated from the CLI's own flags, and copy the exact command for your
    shell, or run it right there. Invalid values are caught before they reach <code>sf</code>.</p>
  </div>
  <img src="{{ '/assets/images/builder.png' | relative_url }}" alt="Command Builder with a package version list form and the resulting command">
</div>

## Also inside

- **Doctor**: checks `sf`, its version, the packaging plugin, and every command's flags; on a newer CLI only a changed command is disabled.
- **Project aliases** from `sfdx-project.json` (read-only): an Alias column, orphan / duplicate / missing warnings, ready-to-paste entries.
- **Installed packages** and **1GP versions**, with released and latest-per-package filters for long lists.
- **History** of every `sf` call with exit code and stderr; `c` or a right-click copies any ID, even over SSH.

<div class="promise" markdown="1">

### What GP Atlas never does

It never creates, promotes, deletes, installs, converts or schedules anything; never logs in or out;
never changes the CLI configuration or your aliases; never runs `sf org display`; never reads auth
files; never stores tokens or installation keys; and makes no network calls of its own.

</div>

<div class="cta" markdown="1">

## Try it in a minute

```bash
curl -fsSL https://raw.githubusercontent.com/holapancho/gp-atlas/main/install.sh | sh
gp-atlas
```

[Install options](install.md) · [User guide](guide.md) · [Beta testing](beta-testing.md) · [Roadmap](https://github.com/holapancho/gp-atlas/blob/main/ROADMAP.md)

</div>

<p style="font-size:.8rem;color:#6b7383;margin-top:2rem">Screenshots use fictional demo data (Acme). Salesforce and related marks are trademarks of Salesforce, Inc.</p>
