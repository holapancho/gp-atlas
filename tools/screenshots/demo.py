"""Fictional demo data (Acme) for fake-sf, used only to take the documentation screenshots.

Usage: demo.py OUT_DIR REPO_ROOT (needs target/debug/gp-atlas-beta and fake-sf)."""
import json, os, sys, subprocess
OUT = sys.argv[1]; REPO = sys.argv[2]
os.makedirs(OUT, exist_ok=True)
F = os.path.join(REPO, "fixtures/sf-2.150.6")
cases = []
def put(name, obj):
    with open(os.path.join(OUT, name), "w") as f: json.dump(obj, f)
def ok(name, result, warnings=()):
    put(name, {"status": 0, "result": result, "warnings": list(warnings)})
def case(argv, name, code=0):
    cases.append({"argv": argv, "stdout": name, **({"exit_code": code} if code else {})})
for n in ["version.json", "plugins.json"]:
    os.system(f"cp {F}/{n} {OUT}/{n}")
case(["version", "--json"], "version.json"); case(["plugins", "--json"], "plugins.json")
put("alias.json", {"status": 0, "result": [], "warnings": []}); case(["alias", "list", "--json"], "alias.json")

def org(alias, user, oid, inst, edition, hub=False, sandbox=False, scratch=False, dhub=False, dorg=False, status="Connected"):
    o = {"orgId": oid, "accessToken": "[REDACTED] Use 'sf org auth show-access-token' to view",
         "instanceUrl": f"https://{inst}.my.salesforce.com", "loginUrl": "https://login.salesforce.com",
         "username": user, "clientId": "PlatformCLI", "isDevHub": hub, "instanceApiVersion": "64.0",
         "namespacePrefix": None, "name": alias, "instanceName": "NA224", "isSandbox": sandbox,
         "isScratch": scratch, "orgEdition": edition, "alias": alias,
         "isDefaultDevHubUsername": dhub, "isDefaultUsername": dorg, "lastUsed": "2026-11-06T10:00:00.000Z"}
    if not scratch: o["connectedStatus"] = status
    else: o.update({"status": "Active", "expirationDate": "2026-11-20"})
    return o
hub = org("acme-devhub", "release.manager@acme.example", "00D5g00000ACMEHUB", "acme", "Enterprise Edition", hub=True, dhub=True)
pkg1 = org("acme-packaging", "packaging@acme.example", "00D5g00000ACMEPKG", "acme-pkg", "Developer Edition")
cust = org("customer-trial", "admin@globex-trial.example", "00D5g00000GLOBEXT", "globex-trial", "Enterprise Edition", dorg=True)
uat = org("acme-uat", "release.manager@acme.example.uat", "00D7A00000ACMEUAT", "acme--uat.sandbox", "Enterprise Edition", sandbox=True)
old = org("old-demo-org", "demo@initech.example", "00D3h00000INITECH", "initech-demo", "Developer Edition",
          status="Unable to refresh session: expired access/refresh token")
scr = org("feature-pricing", "test-pricing@example.com", "00D9E00000SCRATCH", "scratch-pricing", "Developer Edition", scratch=True)
orgs = {"other": [pkg1, cust, old], "sandboxes": [uat], "nonScratchOrgs": [hub, pkg1, cust, uat, old],
        "devHubs": [hub], "scratchOrgs": [scr]}
ok("orgs.json", orgs); case(["org", "list", "--json"], "orgs.json")
case(["org", "list", "--skip-connection-status", "--json"], "orgs.json")

H = "acme-devhub"
P = {"core": ("0Ho5g000000ACORE", "Acme Core", "Managed", "acmecore", "033000000000CORE"),
     "bill": ("0Ho5g000000ABILL", "Acme Billing", "Managed", "acmebill", "033000000000BILL"),
     "conn": ("0Ho5g000000ACONN", "Acme Connectors", "Managed", "acmeconn", "033000000000CONN"),
     "anal": ("0Ho5g000000AANLY", "Acme Analytics", "Unlocked", None, "033000000000ANLY")}
def pid(k): return P[k][0] + "AA"  # 18 chars
rows = [{"Id": pid(k), "SubscriberPackageId": P[k][4] + "AA", "Name": P[k][1], "Description": "",
         "NamespacePrefix": P[k][3], "ContainerOptions": P[k][2], "ConvertedFromPackageId": None, "Alias": "",
         "IsOrgDependent": "N/A" if P[k][2] == "Managed" else "No", "PackageErrorUsername": None,
         "AppAnalyticsEnabled": False, "CreatedBy": "0055g00000ACMEUSR"} for k in ["core", "bill", "conn", "anal"]]
ok("pkgs.json", rows)
case(["package", "list", "--target-dev-hub", H, "--json"], "pkgs.json")
case(["package", "list", "--target-dev-hub", H, "--verbose", "--json"], "pkgs.json")

V = {}  # key -> list of (version, released, ancestor, branch, created)
V["core"] = [("1.0.0.1", True, None, "", "2025-03-04"), ("1.1.0.2", True, "1.0.0.1", "", "2025-05-12"),
             ("1.1.1.1", True, "1.1.0.2", "", "2025-06-02"), ("2.0.0.3", True, "1.1.0.2", "", "2025-09-15"),
             ("2.1.0.2", True, "2.0.0.3", "", "2026-01-20"), ("2.2.0.1", True, "2.1.0.2", "", "2026-04-08"),
             ("2.3.0.4", True, "2.2.0.1", "", "2026-08-27"), ("2.4.0.1", False, "2.3.0.4", "main", "2026-11-03")]
V["bill"] = [("1.0.0.2", True, None, "", "2025-07-01"), ("2.0.0.1", True, "1.0.0.2", "", "2026-02-11"),
             ("2.1.0.3", True, "2.0.0.1", "", "2026-09-30"), ("2.2.0.1", False, "2.1.0.3", "feature/tax", "2026-11-05")]
V["conn"] = [("1.0.0.1", True, None, "", "2025-11-18"), ("1.2.0.1", True, "1.0.0.1", "", "2026-06-14")]
V["anal"] = [("0.9.0.5", False, None, "", "2026-10-21"), ("1.0.0.2", True, None, "", "2026-10-29")]
D = {("core", "1.0.0.1"): "First release", ("core", "1.1.0.2"): "Account sync",
     ("core", "1.1.1.1"): "Fix sync retries", ("core", "2.0.0.3"): "New data model",
     ("core", "2.1.0.2"): "Bulk import", ("core", "2.2.0.1"): "Faster search",
     ("core", "2.3.0.4"): "Audit trail\nand export to CSV", ("core", "2.4.0.1"): "Multi-currency (beta)",
     ("bill", "1.0.0.2"): "Invoices", ("bill", "2.0.0.1"): "Payment plans",
     ("bill", "2.1.0.3"): "Fixes invoice rounding", ("bill", "2.2.0.1"): "Tax engine",
     ("conn", "1.0.0.1"): "REST connector", ("conn", "1.2.0.1"): "Webhooks",
     ("anal", "1.0.0.2"): "Dashboards"}
ids = {}
n = 0
allrows = []
for k, vs in V.items():
    for (ver, rel, anc, br, cd) in vs:
        n += 1
        ids[(k, ver)] = f"04t5g00000{n:04d}{k[:4].upper()}"
for k, vs in V.items():
    out = []
    for (ver, rel, anc, br, cd) in vs:
        ma, mi, pa, bu = map(int, ver.split("."))
        r = {"Package2Id": pid(k), "Branch": br or None, "Tag": None, "MajorVersion": ma, "MinorVersion": mi,
             "PatchVersion": pa, "BuildNumber": bu, "Id": "05i" + ids[(k, ver)][3:],
             "SubscriberPackageVersionId": ids[(k, ver)], "ConvertedFromVersionId": None,
             "Name": f"v{ma}.{mi}", "NamespacePrefix": P[k][3], "Package2Name": P[k][1], "Description": D.get((k, ver)),
             "Version": ver, "IsPasswordProtected": False, "IsReleased": rel,
             "CreatedDate": cd + " 14:02", "LastModifiedDate": cd + " 14:40",
             "InstallUrl": "https://login.salesforce.com/packaging/installPackage.apexp?p0=" + ids[(k, ver)],
             "CodeCoverage": "", "HasPassedCodeCoverageCheck": True, "ValidationSkipped": False,
             "Alias": "", "IsOrgDependent": "N/A", "ReleaseVersion": 64, "BuildDurationInSeconds": 412,
             "HasMetadataRemoved": "No", "CreatedBy": "0055g00000ACMEUSR"}
        if anc and P[k][2] == "Managed":
            r["AncestorId"] = "05i" + ids[(k, anc)][3:]; r["AncestorVersion"] = anc
        out.append(r)
    allrows += out
    ok(f"v-{k}.json", out)
    case(["package", "version", "list", "--target-dev-hub", H, "--packages", pid(k), "--json"], f"v-{k}.json")
ok("v-all.json", allrows); case(["package", "version", "list", "--target-dev-hub", H, "--json"], "v-all.json")
rel = [r for r in allrows if r["IsReleased"]]
ok("v-rel.json", rel); case(["package", "version", "list", "--target-dev-hub", H, "--released", "--json"], "v-rel.json")

# Ancestry (released versions, synthetic root as the CLI prints it, F41)
for k in ["core", "bill", "conn"]:
    lines = ["strict graph G {", '\t nodeunknown [label="root"]']
    edges = []
    for (ver, rel_, anc, br, cd) in V[k]:
        if not rel_: continue
        lines.append(f'\t node{ids[(k, ver)]} [label="{ver}"]')
        edges.append(("unknown" if not anc else ids[(k, anc)], ids[(k, ver)]))
    lines += [f"\t node{a} -- node{b}" for a, b in edges]
    ok(f"anc-{k}.json", "\n".join(lines) + "\n}")
    case(["package", "version", "displayancestry", "--target-dev-hub", H, "--package", pid(k), "--dot-code", "--json"], f"anc-{k}.json")

# Dependencies of Acme Billing 2.1.0.3: Core 2.3.0.4 -> Connectors 1.2.0.1 -> Billing (root-last)
b, c, x = ids[("bill", "2.1.0.3")], ids[("core", "2.3.0.4")], ids[("conn", "1.2.0.1")]
dot = "\n".join(["strict digraph G {",
    f'\t node_{c} [label="Acme Core@2.3.0.4" color="green"]',
    f'\t node_{x} [label="Acme Connectors@1.2.0.1" color="green"]',
    f'\t node_{b} [label="Acme Billing@2.1.0.3" color="green"]',
    f"\t node_{c} -> node_{x}", f"\t node_{c} -> node_{b}", f"\t node_{x} -> node_{b}", "}"])
ok("deps.json", dot)
for k, vs in V.items():
    for (ver, *_r) in vs:
        case(["package", "version", "displaydependencies", "--target-dev-hub", H, "--package", ids[(k, ver)],
              "--edge-direction", "root-last", "--json"], "deps.json")

# Installed in customer-trial: Core older, Connectors same, Billing missing.
inst = [{"Id": "0A35g00000INST01", "SubscriberPackageId": P["core"][4] + "AA", "SubscriberPackageName": "Acme Core",
         "SubscriberPackageNamespace": "acmecore", "SubscriberPackageVersionId": ids[("core", "2.2.0.1")],
         "SubscriberPackageVersionName": "v2.2", "SubscriberPackageVersionNumber": "2.2.0.1"},
        {"Id": "0A35g00000INST02", "SubscriberPackageId": P["conn"][4] + "AA", "SubscriberPackageName": "Acme Connectors",
         "SubscriberPackageNamespace": "acmeconn", "SubscriberPackageVersionId": ids[("conn", "1.2.0.1")],
         "SubscriberPackageVersionName": "v1.2", "SubscriberPackageVersionNumber": "1.2.0.1"}]
ok("inst.json", inst)
for o in ["customer-trial", "acme-uat"]:
    case(["package", "installed", "list", "--target-org", o, "--json"], "inst.json")
ok("empty.json", [], ["No results found"])
for o in ["acme-devhub", "acme-packaging"]:
    case(["package", "installed", "list", "--target-org", o, "--json"], "empty.json")
p1 = [{"MetadataPackageVersionId": f"04t3h0000000{i:02d}LEG", "MetadataPackageId": "0333h0000000LEGACY",
       "Name": f"Acme Legacy {v}", "ReleaseState": s, "Version": v, "BuildNumber": b_}
      for i, (v, s, b_) in enumerate([("3.0", "Released", 1), ("3.1", "Beta", 1), ("3.1", "Released", 2), ("3.2", "Released", 1)])]
ok("p1.json", p1); case(["package1", "version", "list", "--target-org", "acme-packaging", "--json"], "p1.json")

def err(name, msg, code=1):
    return {"name": name, "message": msg, "exitCode": code, "context": "Command", "stack": name, "cause": "undefined",
            "warnings": [], "code": name, "status": code, "commandName": "Command"}
put("invalid-type.json", err("INVALID_TYPE", "sObject type 'MetadataPackageVersion' is not supported. If you are attempting to use a custom object, be sure to append the '__c' after the entity name."))
put("http420.json", err("ERROR_HTTP_420", "Unexpected HTML response (HTTP 420): the org could not be reached."))
put("expired.json", err("RefreshTokenAuthError", "Error authenticating with the refresh token due to: expired access/refresh token"))
open(os.path.join(OUT, "index.json"), "w").write(json.dumps({"cases": cases}))

# Probes: ask gp-atlas-beta for the exact probe argv of every org.
beta = os.path.join(REPO, "target/debug/gp-atlas-beta"); fsf = os.path.join(REPO, "target/debug/fake-sf")
env = dict(os.environ, GP_ATLAS_FAKE_SF_INDEX=os.path.join(OUT, "index.json"))
for o in ["acme-devhub", "acme-packaging", "customer-trial", "acme-uat", "old-demo-org", "feature-pricing"]:
    txt = subprocess.run([beta, "--sf", fsf, "show", "probes", "--org", o], capture_output=True, text=True, env=env).stdout
    for line in txt.splitlines():
        if "sf " not in line: continue
        argv = line[line.index("sf ") + 3:].split()
        if "--json" not in argv: argv.append("--json")
        if any(c["argv"] == argv for c in cases): continue
        name = {"old-demo-org": "expired.json", "feature-pricing": "http420.json"}.get(o)
        if name is None:
            if argv[0] == "package1" and o != "acme-packaging": name = "invalid-type.json"
            elif argv[:3] == ["package", "version", "list"] or argv[:4] == ["package", "version", "create", "list"]:
                name = "v-rel.json" if argv[:3] == ["package", "version", "list"] else "empty.json"
            else: name = "empty.json"
        cases.append({"argv": argv, "stdout": name, **({"exit_code": 1} if name.endswith(("type.json", "420.json", "expired.json")) else {})})
open(os.path.join(OUT, "index.json"), "w").write(json.dumps({"cases": cases}))
print(len(cases), "cases")
