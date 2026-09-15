#!/usr/bin/env bash
# FORGE new-slice — start a vertical slice. Creates .forge/slices/<id>/ with one
# evidence stub per phase (each marked FORGE-STUB so the gate treats it as "not
# started"), sets it as the current slice, and appends it to state/backlog.
# Run from the project root:  .forge/new-slice.sh <slug>
set -euo pipefail

SLUG="${1:-}"
if [ -z "$SLUG" ]; then
  echo "usage: .forge/new-slice.sh <slug>   (e.g. .forge/new-slice.sh login-form)" >&2
  exit 1
fi
SLUG="$(printf '%s' "$SLUG" | tr '[:upper:] ' '[:lower:]-' | tr -cd 'a-z0-9-')"
if [ -z "$SLUG" ]; then
  echo "usage: slug is empty after sanitising — use letters/digits (a-z, 0-9)." >&2; exit 1
fi

F=".forge"
[ -d "$F" ] || { echo "no .forge/ here — run bootstrap first." >&2; exit 1; }
python3 -c 'import json,sys; json.load(open(".forge/state.json"))' 2>/dev/null \
  || { echo "new-slice: .forge/state.json is not valid JSON — fix it first." >&2; exit 3; }

N="$(python3 - <<'PY'
import json
print(len(json.load(open(".forge/state.json")).get("slices", [])) + 1)
PY
)"
ID="S${N}-${SLUG}"
D="$F/slices/$ID"
[ -d "$D" ] && { echo "slice already exists: $D" >&2; exit 1; }
mkdir -p "$D"

MARK='<!-- FORGE-STUB: replace this file with real evidence, then delete this line. -->'

# stub <file> <body...>
stub() { local f="$D/$1"; shift; { echo "$MARK"; printf '%s\n' "$@"; } > "$f"; }

stub slice.md \
  "# $ID — SLICE (DISCOVER + PLAN)" "" \
  "## Delivers (one sentence, end-to-end)" "_What a user can see/do after this ships._" "" \
  "## Acceptance criteria (testable — the tester will run these)" "- [ ] " "" \
  "## Approach + why this slice now"

stub design.md \
  "# $ID — DESIGN" \
  "Files to touch, interfaces, data flow, and existing code reused (with paths)." \
  "Scope to THIS slice. 'N/A — <reason>' is a valid recorded answer, not a skip."

stub implement.md \
  "# $ID — IMPLEMENT (claim)" \
  "Author: implementer agent. Every file changed, how to run it, known gaps." \
  "Do not describe work not done — a different agent will run this."

stub test.md \
  "# $ID — TEST (evidence)" \
  "Author: a DIFFERENT agent than the implementer. Run each acceptance criterion" \
  "as a real command; paste actual output; probe one edge case." \
  "<!-- Finish with a line beginning VERDICT: then PASS, or FAIL + root-cause. -->"

stub review.md \
  "# $ID — REVIEW (evidence)" \
  "Production-standards: correctness, error handling, tests on critical paths," \
  "dead code, AI-code tells. Findings with file:line." \
  "<!-- Finish with a line beginning VERDICT: then PASS, or FAIL + reasons. -->"

stub security.md \
  "# $ID — SECURITY (evidence)" \
  "Security pass over what this slice touched: injection, secrets, authz, unsafe" \
  "defaults, deps. Each finding: severity, file:line, exploit path, fix." \
  "Critical/High become their own new slices." \
  "<!-- Finish with a line beginning VERDICT: then PASS, or FAIL + findings. -->"

stub document.md \
  "# $ID — DOCUMENT" \
  "What docs/README/changelog changed for this slice, and where."

stub release.md \
  "# $ID — RELEASE" \
  "Authorised only after 'python3 .forge/gate.py $ID' exits 0." \
  "Record: branch, commit sha / PR link, and anything only the human can do."

python3 - "$ID" "$SLUG" <<'PY'
import json, sys, datetime
sid, slug = sys.argv[1], sys.argv[2]
s = json.load(open(".forge/state.json"))
s["currentSlice"] = sid
s.setdefault("slices", []).append(
    {"id": sid, "title": slug, "status": "in-progress",
     "started": datetime.date.today().isoformat()})
with open(".forge/state.json", "w") as f:
    json.dump(s, f, indent=2); f.write("\n")
PY

echo "started $ID  (current slice)"
echo "  evidence stubs in $D/  (each marked FORGE-STUB until you replace it)"
echo "  gate: python3 .forge/gate.py"
