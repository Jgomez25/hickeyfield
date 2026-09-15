#!/usr/bin/env python3
"""FORGE gate — the evidence check that authorises RELEASE for a slice.

Claims are worthless; evidence files are the only proof of done. This script
does not trust status text — it reads the per-phase evidence files on disk and
enforces that real work was recorded and the quality gates actually passed.

Usage (run from the project root that contains .forge/):
  python3 .forge/gate.py                 # check the current slice (state.json)
  python3 .forge/gate.py <sliceId>       # check a specific slice
  python3 .forge/gate.py --all           # one-line status for every slice

A phase counts as done only when its evidence file exists, no longer contains
the FORGE-STUB marker (i.e. the stub was replaced with real content), and — for
the quality gates TEST/REVIEW/SECURITY — carries a non-comment line beginning
'VERDICT:' whose verdict is PASS. Exit 0 = release-ready; 1 = not ready
(prints the gaps); 2 = harness/setup error.
"""
import json
import os
import sys

FORGE = ".forge"
STUB_MARKER = "FORGE-STUB"
MIN_EVIDENCE_CHARS = 30  # after stripping comments/markers

# (label, evidence filename, requires a passing VERDICT line)
SLICE_PHASES = [
    ("SLICE  (discover+plan)", "slice.md", False),
    ("DESIGN", "design.md", False),
    ("IMPLEMENT", "implement.md", False),
    ("TEST", "test.md", True),
    ("REVIEW", "review.md", True),
    ("SECURITY", "security.md", True),
    ("DOCUMENT", "document.md", False),
]


def load_state():
    p = os.path.join(FORGE, "state.json")
    if not os.path.exists(p):
        print(f"gate: no {p} — run bootstrap first."); sys.exit(2)
    try:
        with open(p) as f:
            return json.load(f)
    except Exception as e:
        print(f"gate: cannot read {p}: {e}"); sys.exit(2)


def _content_lines(txt):
    """Non-blank lines that are not whole-line HTML comments."""
    out = []
    for ln in txt.splitlines():
        s = ln.strip()
        if not s or s.startswith("<!--"):
            continue
        out.append(s)
    return out


def phase_state(txt, needs_verdict):
    if STUB_MARKER in txt:
        return ("stub", "not started", False)
    lines = _content_lines(txt)
    if len("\n".join(lines)) < MIN_EVIDENCE_CHARS:
        return ("thin", "evidence too short", False)
    if needs_verdict:
        verdicts = [l for l in lines if l.upper().startswith("VERDICT:")]
        if not verdicts:
            return ("?", "no 'VERDICT:' line", False)
        last = verdicts[-1].upper()
        if "FAIL" in last:
            return ("FAIL", "repair required", False)
        if "PASS" in last:
            return ("PASS", "", True)
        return ("?", "VERDICT line not PASS/FAIL", False)
    return ("ok", "", True)


def project_preamble():
    problems = []
    for f in ("objective.md", "backlog.md"):
        p = os.path.join(FORGE, f)
        if not os.path.exists(p):
            problems.append(f"project {f} missing"); continue
        txt = open(p).read()
        if STUB_MARKER in txt or len("\n".join(_content_lines(txt))) < MIN_EVIDENCE_CHARS:
            problems.append(f"project {f} still a stub / too thin (DISCOVER/PLAN incomplete)")
    return problems


def check_slice(sid):
    d = os.path.join(FORGE, "slices", sid)
    if not os.path.isdir(d):
        return False, [("(slice dir)", "MISSING", d)]
    rows, ok = [], True
    for label, fn, needs_verdict in SLICE_PHASES:
        path = os.path.join(d, fn)
        if not os.path.exists(path):
            rows.append((label, "missing", "")); ok = False; continue
        status, note, good = phase_state(open(path).read(), needs_verdict)
        rows.append((label, status, note)); ok = ok and good
    return ok, rows


def print_slice(sid):
    ok, rows = check_slice(sid)
    print(f"\nSlice {sid}: {'RELEASE-READY ✅' if ok else 'NOT READY ⛔'}")
    for label, status, note in rows:
        print(f"  {label:<24} {status:<8} {note}")
    return ok


def main():
    args = sys.argv[1:]
    st = load_state()
    pre = project_preamble()
    if pre:
        print("GATE FAIL (project setup):")
        for p in pre:
            print(f"  - {p}")

    if "--all" in args:
        slices = st.get("slices", [])
        allok = not pre
        if not slices:
            print("  (no slices yet — run DISCOVER/PLAN, then .forge/new-slice.sh)")
            allok = False
        for s in slices:
            sid, status = s.get("id"), s.get("status", "?")
            ok, _ = check_slice(sid)
            allok = allok and (ok or status == "released")
            print(f"  {sid:<28} status={status:<12} {'ready ✅' if ok else 'not-ready'}")
        sys.exit(0 if allok else 1)

    sid = next((a for a in args if not a.startswith("-")), st.get("currentSlice"))
    if not sid:
        print("gate: no current slice set in .forge/state.json."); sys.exit(2)
    ok = print_slice(sid)
    print()
    if ok and not pre:
        print(f"RELEASE authorised for {sid}: all phases evidenced, quality gates PASS.")
        sys.exit(0)
    print(f"RELEASE blocked for {sid}: resolve the rows above — each gap needs real evidence.")
    sys.exit(1)


if __name__ == "__main__":
    main()
