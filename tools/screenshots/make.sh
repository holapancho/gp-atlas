#!/bin/sh
# Regenerates docs/assets/images/*.png from fictional demo data (Acme).
# Needs: a debug build (cargo build -p gp-atlas-tui -p gp-atlas-cli -p fake-sf),
# python3 with pyte (pip install pyte), node with playwright and Chromium.
# Usage: tools/screenshots/make.sh   (run from the repository root)
set -eu
R="$(pwd)"; T="$R/tools/screenshots"; W="${TMPDIR:-/tmp}/gp-atlas-shots"
rm -rf "$W" && mkdir -p "$W/demo/proj" "$W/out"
python3 "$T/demo.py" "$W/demo" "$R" >/dev/null
python3 - "$W/demo" <<'PY'
import json, sys
d = sys.argv[1]
idx = json.load(open(f"{d}/index.json"))
core = sorted({c["argv"][6] for c in idx["cases"] if c["argv"][:3] == ["package", "version", "displaydependencies"] and c["argv"][6].endswith("CORE")})
vers = ["1.0.0-1", "1.1.0-2", "1.1.1-1", "2.0.0-3", "2.1.0-2", "2.2.0-1", "2.3.0-4"]
json.dump({"packageDirectories": [{"path": "force-app", "package": "AcmeCore", "default": True}],
           "packageAliases": {"AcmeCore": "0Ho5g000000ACOREAA", "AcmeBilling": "0Ho5g000000ABILLAA",
                              **{f"AcmeCore@{v}": i for v, i in zip(vers, core)}}},
          open(f"{d}/proj/sfdx-project.json", "w"), indent=2)
PY
want() { python3 -c "import json,sys; print(json.dumps(json.load(open('$T/shots.json'))['$1']['$2']))"; }
for set in packages ancestry short builder; do
  rm -rf "$W/demo/cache" "$W/out"/*.json
  (cd "$W/demo/proj" && GP_ATLAS_CACHE_DIR="$W/demo/cache" GP_ATLAS_FAKE_SF_INDEX="$W/demo/index.json" \
    OUT="$W/out" ROWS="$(want $set rows)" STEPS="$(want $set steps)" \
    python3 "$T/capture.py" "$R/target/debug/gp-atlas" --sf "$R/target/debug/fake-sf")
  case $set in
    packages) keep="packages" ;; ancestry) keep="ancestry" ;;
    short) keep="dependencies access" ;; builder) keep="builder" ;;
  esac
  list=""
  for n in $keep; do
    python3 "$T/render.py" "$W/out/$n.json" "$W/out/$n.html" "gp-atlas — ~/acme/core"
    list="$list[\"$W/out/$n.html\",\"$R/docs/assets/images/$n.png\"],"
  done
  NODE_PATH="$(npm root -g)" node "$T/snap.js" "[${list%,}]"
done
echo "updated docs/assets/images"
